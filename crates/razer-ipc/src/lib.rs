//! Shared worker wire contract and owned child-process client. No DLL or HID implementation.
use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

#[cfg(windows)]
#[path = "runtime_job.rs"]
mod job;
mod worker_diagnostics;
mod worker_shutdown;
pub use worker_shutdown::ShutdownConfirmation;

pub const FRAME_PREFIX: &str = "RAZER_UI_SERVICE ";
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
pub mod analog_observation;
pub mod controller_sleep;
pub mod controller_initialization;
pub mod controller_socd;
pub mod device_tasks;
pub mod gamepad_observation;
pub mod global_shortcut_capture;
pub mod input_observation;
pub mod keyboard_onboard_profiles;
pub mod keyboard_profile_inventory;
pub mod keypad_observation;
pub mod macro_recording;
pub mod mapping_control;
pub mod mapping_observation;
pub mod mouse_input_monitoring;
pub mod mouse_rotation;
pub mod shortcuts;

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
mod hanbo_cooler_tests;
#[cfg(test)]
mod help_system_info_tests;
#[cfg(test)]
mod hyperpolling_receiver_tests;
#[cfg(test)]
mod mouse_scroll_tests;
#[cfg(test)]
mod mouse_tracking_tests;
#[cfg(test)]
mod system_monitor_tests;
#[cfg(test)]
mod thx_tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HanboCoolerObservation {
    pub node: razer_device::backend::HidNode,
    pub product_id: u32,
    pub telemetry: razer_device::hanbo_cooler::Telemetry,
}

impl HanboCoolerObservation {
    pub fn validate_for(&self, node: &razer_device::backend::HidNode) -> anyhow::Result<()> {
        razer_device::hanbo_cooler::validate_node(node, self.product_id)?;
        anyhow::ensure!(
            self.node == *node,
            "Hanbo observation belongs to a different collection"
        );
        self.telemetry.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HanboStartupObservation {
    pub node: razer_device::backend::HidNode,
    pub product_id: u32,
    pub reading: razer_device::hanbo_cooler::StartupReading,
}

impl HanboStartupObservation {
    pub fn validate_for(
        &self,
        node: &razer_device::backend::HidNode,
        query: razer_device::hanbo_cooler::StartupQuery,
    ) -> anyhow::Result<()> {
        razer_device::hanbo_cooler::validate_node(node, self.product_id)?;
        anyhow::ensure!(
            self.node == *node,
            "Hanbo startup observation belongs to a different collection"
        );
        anyhow::ensure!(
            self.reading.query() == query,
            "Hanbo startup getter response changed"
        );
        Ok(())
    }
}

/// `operation` is the internal wire tag, so no variant payload may declare a
/// field named `operation`; payloads carry their own operation instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ServiceRequest {
    HyperpollingReceiver {
        target: razer_device::hyperpolling_receiver::Target,
        command: razer_device::hyperpolling_receiver::Command,
        transaction: u8,
    },
    MouseInputMonitoringStart {
        owner_guid: String,
    },
    MouseInputMonitoringPoll {
        owner_guid: String,
    },
    MouseInputMonitoringStop {
        owner_guid: String,
    },
    HidNodeHanboStartupRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        query: razer_device::hanbo_cooler::StartupQuery,
    },
    SystemMonitorStart {
        owner_guid: String,
        container_id: String,
        product_id: u32,
    },
    SystemMonitorPoll {
        owner_guid: String,
    },
    SystemMonitorStop {
        owner_guid: String,
    },
    MouseTrackingStart {
        operation_id: String,
        target: razer_device::mouse_tracking::Target,
        setting: razer_device::mouse_tracking::TrackingSetting,
    },
    MouseTrackingPoll {
        operation_id: String,
    },
    MouseTrackingCancel {
        operation_id: String,
    },
    MousePhilipsStart {
        operation_id: String,
        target: razer_device::feature_submission::Target,
        setting: razer_device::mouse_philips::Setting,
    },
    MousePhilipsPoll {
        operation_id: String,
    },
    MousePhilipsCancel {
        operation_id: String,
    },
    MousePhilipsMeasurementStart {
        operation_id: String,
        target: razer_device::device_reads::DeviceReadTarget,
        kind: razer_device::mouse_philips::measurement::Kind,
    },
    MousePhilipsMeasurementPoll {
        operation_id: String,
    },
    MousePhilipsMeasurementCancel {
        operation_id: String,
    },
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
    /// Current AudioEffectsTHXCarol owner for 1392/1442/3942. Settings are the
    /// stored comboMode preferences used only to seed the runtime cache.
    SurroundOpen {
        identity: razer_device::audio_surround::Identity,
        settings: razer_device::audio_surround::Settings,
    },
    SurroundChange {
        identity: razer_device::audio_surround::Identity,
        change: razer_device::audio_surround::Change,
    },
    SurroundObserve {
        identity: razer_device::audio_surround::Identity,
    },
    SurroundClose {
        identity: razer_device::audio_surround::Identity,
    },
    HidNodeMouseCurrentPower {
        node: razer_device::backend::HidNode,
        product_id: u32,
        #[serde(default)]
        peer_product_id: Option<u32>,
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
    BleMouseRotation {
        target: razer_device::mouse_runtime_artwork::WindowsBleTarget,
        change: Option<razer_device::mouse_rotation::Setting>,
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
    ReceiverLightingApply {
        operation_id: String,
        path: String,
        device_container_id: String,
        configuration: razer_device::receiver_lighting::Configuration,
    },
    ReceiverLightingPoll {
        operation_id: String,
    },
    ReceiverLightingChromaAttributeResult {
        operation_id: String,
        worker_id: uuid::Uuid,
        request_id: u64,
        receipt: razer_device::receiver_lighting::ChromaAttributeReceipt,
    },
    ReceiverLightingStop {
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
    ControllerLighting {
        node: razer_device::backend::HidNode,
        product_id: u32,
        lighting_operation: razer_device::controller_lighting::LightingOperation,
    },
    MarciLighting {
        device_container_id: String,
        owner_id: String,
        lighting_operation: razer_device::marci_lighting::Operation,
    },
    ControllerSleep {
        node: razer_device::backend::HidNode,
        product_id: u32,
        sleep_operation: controller_sleep::SleepOperation,
    },
    ControllerInitialization {
        node: razer_device::backend::HidNode,
        product_id: u32,
        plan: razer_device::controller_initialization::InitializationPlan,
    },
    ControllerSocd {
        node: razer_device::backend::HidNode,
        product_id: u32,
        socd_operation: controller_socd::SocdOperation,
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
    /// Source `rzDeviceElsa` Output/Interrupt exchange for Hanbo AIO 3893.
    /// The service validates the retained HID collection and a matching
    /// status-1 reply before reporting acknowledgement.
    HidNodeHanboCooler {
        node: razer_device::backend::HidNode,
        product_id: u32,
        command: razer_device::hanbo_cooler::CoolerCommand,
    },
    /// The source publishes one sample after its fan and pump getters finish.
    HidNodeHanboCoolerRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
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
    HidNodeMouseBrightness {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
        submission: razer_device::mouse_brightness::Submission,
    },
    HidNodeMouseDynamic {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
        change: Option<razer_device::mouse_dynamic::Setting>,
    },
    HidNodeMouseCurrentPolling {
        node: razer_device::backend::HidNode,
        product_id: u32,
        change: Option<razer_device::mouse_current_polling::Change>,
    },
    HidNodeMouseRotation {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
        change: Option<razer_device::mouse_rotation::Setting>,
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
    KeyboardProfileInventoryRead {
        target: keyboard_profile_inventory::Target,
    },
    KeyboardOnboardProfilesRead {
        target: keyboard_profile_inventory::Target,
        options: razer_device::keyboard_onboard_profiles::ReadOptions,
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
    /// Current mounted Help's local Blade display or complete report query.
    HelpSystemInfo {
        product_id: u32,
        query: razer_device::system_info::Query,
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
    DeviceMouseBrightness {
        target: razer_device::device_reads::DeviceReadTarget,
        submission: razer_device::mouse_brightness::Submission,
    },
    DeviceMouseDynamic {
        target: razer_device::device_reads::DeviceReadTarget,
        change: Option<razer_device::mouse_dynamic::Setting>,
    },
    DeviceMouseCurrentPolling {
        target: razer_device::device_reads::DeviceReadTarget,
        change: Option<razer_device::mouse_current_polling::Change>,
    },
    DeviceMouseRotation {
        target: razer_device::device_reads::DeviceReadTarget,
        change: Option<razer_device::mouse_rotation::Setting>,
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
    MappingRotationStart {
        owner_guid: String,
        target: mouse_rotation::NativeTarget,
    },
    MappingRotationRequest {
        owner_guid: String,
        request: razer_device::mapping_native_rotation::RotationRequest,
    },
    MappingRotationPoll {
        owner_guid: String,
    },
    MappingRotationStop {
        owner_guid: String,
    },
    MappingGamepadObservationStart {
        target: gamepad_observation::Target,
        observer_guid: String,
        registration: gamepad_observation::Registration,
    },
    MappingGamepadObservationPoll {
        observer_guid: String,
    },
    MappingGamepadObservationStop {
        observer_guid: String,
    },
    MappingKeypadObservationStart {
        target: keypad_observation::Target,
        observer_guid: String,
    },
    MappingKeypadObservationPoll {
        observer_guid: String,
    },
    MappingKeypadObservationStop {
        observer_guid: String,
    },
    MappingAnalogObservationStart {
        target: analog_observation::Target,
        observer_guid: String,
        registration: analog_observation::Registration,
    },
    MappingAnalogObservationPoll {
        observer_guid: String,
    },
    MappingAnalogObservationStop {
        observer_guid: String,
    },
    MappingObservationStart {
        target: mapping_observation::Target,
        observer_guid: String,
    },
    MappingInputObservationStart {
        target: input_observation::Target,
        observer_guid: String,
    },
    MappingInputObservationPoll {
        observer_guid: String,
    },
    MappingInputObservationStop {
        observer_guid: String,
    },
    MappingObservationPoll {
        observer_guid: String,
    },
    MappingObservationStop {
        observer_guid: String,
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
    MacroRecordingMode,
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
    StartGlobalShortcutCapture,
    StartKeyboardCalibrationCapture {
        product_id: u32,
        container_id: String,
    },
    MappingKeyCaptureEvents,
    GlobalShortcutCaptureEvents,
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
    GlobalShortcutStart {
        owner_guid: String,
    },
    GlobalShortcutPublish {
        owner_guid: String,
        revision: u64,
        document: razer_model::shortcuts::CompiledDocument,
    },
    GlobalShortcutPoll {
        owner_guid: String,
    },
    GlobalShortcutStop {
        owner_guid: String,
    },
    MappingExecution {
        owner_guid: String,
        revision: u64,
        target: mapping_control::Target,
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

/// A single long-lived worker and its pipe threads. Construction, requests and
/// destruction perform blocking I/O and belong to the caller's I/O worker.
pub struct ServiceClient {
    child: Option<Child>,
    #[cfg(windows)]
    job: Option<job::WorkerJob>,
    requests: Option<Sender<Vec<u8>>>,
    responses: Receiver<anyhow::Result<ResponseEnvelope>>,
    input_worker: Option<JoinHandle<anyhow::Result<()>>>,
    output_worker: Option<JoinHandle<anyhow::Result<()>>>,
    diagnostic_worker: Option<JoinHandle<anyhow::Result<()>>>,
    diagnostics: worker_diagnostics::WorkerDiagnostics,
    timeout: Duration,
    next_id: u64,
    stopped: bool,
    shutdown_confirmation: Option<ShutdownConfirmation>,
    philips_measurement_started: bool,
    request_guard: Option<RequestGuard>,
}

enum RequestGuard {
    Active(std::sync::Arc<std::sync::atomic::AtomicBool>),
    Canceled(std::sync::Arc<std::sync::atomic::AtomicBool>),
    Scoped {
        active: std::sync::Arc<std::sync::atomic::AtomicBool>,
        canceled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    },
}

impl RequestGuard {
    fn canceled(&self) -> bool {
        use std::sync::atomic::Ordering;
        match self {
            Self::Active(signal) => !signal.load(Ordering::Acquire),
            Self::Canceled(signal) => signal.load(Ordering::Acquire),
            Self::Scoped { active, canceled } => {
                !active.load(Ordering::Acquire) || canceled.load(Ordering::Acquire)
            }
        }
    }
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
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: do not expose a console for a background service.
            command.creation_flags(0x0800_0000);
        }
        let child = command.spawn().context("无法启动设备服务子进程")?;
        let (requests, pending) = mpsc::channel::<Vec<u8>>();
        let (sender, responses) = mpsc::channel();
        let mut client = Self {
            child: Some(child),
            #[cfg(windows)]
            job: Some(job),
            requests: Some(requests),
            responses,
            input_worker: None,
            output_worker: None,
            diagnostic_worker: None,
            diagnostics: worker_diagnostics::WorkerDiagnostics::default(),
            // First registration can perform initialization, event registration,
            // and registration itself (three bounded native callbacks).
            timeout: Duration::from_secs(15),
            next_id: 1,
            stopped: false,
            shutdown_confirmation: None,
            philips_measurement_started: false,
            request_guard: None,
        };
        #[cfg(windows)]
        if let Err(error) = client
            .job
            .as_ref()
            .expect("owned worker job")
            .assign(client.child.as_ref().expect("owned worker process"))
        {
            return Err(client.stop_after_error(error));
        }
        let child = client.child.as_mut().expect("owned worker process");
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(client.stop_after_error(anyhow::anyhow!("设备服务通信管道不可用")));
        };
        let diagnostics = client.diagnostics.clone();
        client.diagnostic_worker = Some(
            match thread::Builder::new()
                .name("razer-service-diagnostics".into())
                .spawn(move || diagnostics.capture_stderr(stderr))
            {
                Ok(worker) => worker,
                Err(error) => {
                    return Err(client.stop_after_error(
                        anyhow::Error::new(error).context("无法创建设备服务诊断线程"),
                    ));
                }
            },
        );
        // Writing a pipe can block if startup hangs before the worker reads it.
        // Keep it off the request thread so recv_timeout covers the full request.
        client.input_worker = Some(
            match thread::Builder::new()
                .name("razer-service-input".into())
                .spawn(move || forward_requests(stdin, pending))
            {
                Ok(worker) => worker,
                Err(error) => {
                    return Err(client.stop_after_error(
                        anyhow::Error::new(error).context("无法创建设备服务输入线程"),
                    ));
                }
            },
        );
        let diagnostics = client.diagnostics.clone();
        client.output_worker = Some(
            match thread::Builder::new()
                .name("razer-service-output".into())
                .spawn(move || {
                    let mut reader = BufReader::new(stdout);
                    forward_responses(&mut reader, sender, &diagnostics)
                }) {
                Ok(worker) => worker,
                Err(error) => {
                    return Err(client.stop_after_error(
                        anyhow::Error::new(error).context("无法创建设备服务输出线程"),
                    ));
                }
            },
        );
        Ok(client)
    }

    /// Request admission has closed. Any cleanup failure was returned to its
    /// caller; unreaped resources remain owned for another cleanup attempt.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// The service's matching successful teardown reply remains available even
    /// when subsequent process reaping or pipe joins fail. Admission closure
    /// and OS termination alone cannot establish this confirmation.
    pub fn shutdown_confirmation(&self) -> Option<ShutdownConfirmation> {
        self.shutdown_confirmation
    }

    /// Confirm that the owned process has exited and all pipe workers joined.
    /// This closes admission without acknowledging native service shutdown or
    /// any in-flight device write. Unreaped handles remain owned on failure so
    /// the caller's I/O worker can retry termination.
    pub fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.stop()
    }

    /// An operation owns this worker exclusively. Invalidating its scope closes
    /// admission and drains its shutdown before releasing the owned process. An
    /// already submitted device command cannot be rolled back by cancellation.
    pub fn set_request_guard(&mut self, guard: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        self.request_guard = Some(RequestGuard::Active(guard));
    }

    /// Observe the operation owner's cancellation flag directly. This shares
    /// request admission, in-flight termination and cleanup with active guards.
    pub fn set_cancellation_guard(
        &mut self,
        canceled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        self.request_guard = Some(RequestGuard::Canceled(canceled));
    }

    /// A retained connection can be canceled while one operation has its own
    /// narrower validity guard. Either owner closes the same request admission.
    pub fn set_scoped_request_guard(
        &mut self,
        active: std::sync::Arc<std::sync::atomic::AtomicBool>,
        canceled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        self.request_guard = Some(RequestGuard::Scoped { active, canceled });
    }

    fn request_scope_canceled(&self) -> bool {
        self.request_guard
            .as_ref()
            .is_some_and(RequestGuard::canceled)
    }

    pub fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        if self.stopped {
            bail!("设备服务已停止，请重新连接");
        }
        let shutting_down = matches!(request, ServiceRequest::Shutdown);
        if !shutting_down && self.request_scope_canceled() {
            return Err(
                self.stop_after_error(anyhow::anyhow!("设备操作范围已取消，独占服务已关闭"))
            );
        }
        let _device_task = request
            .synchronous_device_task()?
            .map(|scope| {
                device_tasks::Registry::global().shared(scope, || self.request_scope_canceled())
            })
            .transpose()?;
        let starting_recorder = matches!(
            request,
            ServiceRequest::StartMacroRecording
                | ServiceRequest::PrepareOtfmRecorder
                | ServiceRequest::StartOtfmRecording
        );
        let controller_initialization = matches!(&request, ServiceRequest::ControllerInitialization { .. });
        let controller_triggers = matches!(
            &request,
            ServiceRequest::ControllerTriggers { .. }
                | ServiceRequest::ControllerLighting { .. }
                | ServiceRequest::MarciLighting { .. }
                | ServiceRequest::ControllerSleep { .. }
                | ServiceRequest::ControllerSocd { .. }
                | ServiceRequest::ControllerInitialization { .. }
                | ServiceRequest::ReceiverLightingApply { .. }
                | ServiceRequest::ReceiverLightingStop { .. }
        );
        let starting_mapping = matches!(
            request,
            ServiceRequest::MappingDeviceStart { .. }
                | ServiceRequest::MappingRotationStart { .. }
                | ServiceRequest::ControllerInputStart { .. }
                | ServiceRequest::MappingOtfmInputStart { .. }
        );
        let starting_key_capture = matches!(
            request,
            ServiceRequest::StartMappingKeyCapture
                | ServiceRequest::MouseInputMonitoringStart { .. }
                | ServiceRequest::StartGlobalShortcutCapture
                | ServiceRequest::StartKeyboardCalibrationCapture { .. }
        );
        let publishing_shortcuts = matches!(request, ServiceRequest::GlobalShortcutPublish { .. });
        let philips_measurement = matches!(
            request,
            ServiceRequest::MousePhilipsMeasurementStart { .. }
                | ServiceRequest::MousePhilipsMeasurementPoll { .. }
                | ServiceRequest::MousePhilipsMeasurementCancel { .. }
        );
        self.philips_measurement_started |= philips_measurement;
        let writing_app_files =
            matches!(&request, ServiceRequest::UserAppFiles { command } if command.mutates_files());
        let writing_device = matches!(
            request,
            ServiceRequest::DeviceWrite { .. }
                | ServiceRequest::HyperpollingReceiver {
                    command: razer_device::hyperpolling_receiver::Command::SetIndicator { .. }
                        | razer_device::hyperpolling_receiver::Command::CancelScan,
                    ..
                }
                | ServiceRequest::MouseTrackingStart { .. }
                | ServiceRequest::MouseTrackingPoll { .. }
                | ServiceRequest::MouseTrackingCancel { .. }
                | ServiceRequest::MousePhilipsStart { .. }
                | ServiceRequest::MousePhilipsPoll { .. }
                | ServiceRequest::MousePhilipsCancel { .. }
                | ServiceRequest::MousePhilipsMeasurementStart { .. }
                | ServiceRequest::MousePhilipsMeasurementPoll { .. }
                | ServiceRequest::MousePhilipsMeasurementCancel { .. }
                | ServiceRequest::HidNodeMouseScroll { .. }
                | ServiceRequest::DeviceMouseScroll { .. }
                | ServiceRequest::HidNodeSeirenEq { .. }
                | ServiceRequest::DeviceSeirenEq { .. }
                | ServiceRequest::SeirenNativeOpen { .. }
                | ServiceRequest::SeirenNativeCall { .. }
                | ServiceRequest::SeirenNativeEvents { .. }
                | ServiceRequest::SeirenNativeClose { .. }
                | ServiceRequest::SurroundOpen { .. }
                | ServiceRequest::SurroundChange { .. }
                | ServiceRequest::SurroundClose { .. }
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
                | ServiceRequest::HidNodeMouseDynamic {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::DeviceMouseDynamic {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::HidNodeMouseRotation {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::DeviceMouseRotation {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::BleMouseRotation {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::HidNodeMouseCurrentPolling {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::DeviceMouseCurrentPolling {
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
                | ServiceRequest::MappingRotationStart { .. }
                | ServiceRequest::MappingRotationRequest { .. }
                | ServiceRequest::MappingRotationPoll { .. }
                | ServiceRequest::MappingRotationStop { .. }
                | ServiceRequest::SetOtfmMappingsSuspended { .. }
                | ServiceRequest::MacroRecorderGlobalState
                | ServiceRequest::MacroRecorderStorageWrite { .. }
                | ServiceRequest::StartMappingKeyCapture
                | ServiceRequest::StartGlobalShortcutCapture
                | ServiceRequest::StartKeyboardCalibrationCapture { .. }
                | ServiceRequest::MappingKeyCaptureEvents
                | ServiceRequest::GlobalShortcutCaptureEvents
                | ServiceRequest::StopMappingKeyCapture
                | ServiceRequest::StartShortcutCapture
                | ServiceRequest::HidNodeWrite { .. }
                | ServiceRequest::HidNodeHanboCooler { .. }
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
            ServiceRequest::HidNodeMouseBrightness { submission, .. }
                | ServiceRequest::DeviceMouseBrightness { submission, .. } if submission.is_write()
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
        let reading_keyboard_inventory = matches!(
            request,
            ServiceRequest::KeyboardProfileInventoryRead { .. }
                | ServiceRequest::KeyboardOnboardProfilesRead { .. }
        );
        let reading_device = matches!(
            request,
            ServiceRequest::DeviceRead { .. }
                | ServiceRequest::HyperpollingReceiver {
                    command: razer_device::hyperpolling_receiver::Command::Serial
                        | razer_device::hyperpolling_receiver::Command::Edition
                        | razer_device::hyperpolling_receiver::Command::Indicator
                        | razer_device::hyperpolling_receiver::Command::Bindings,
                    ..
                }
                | ServiceRequest::SystemMonitorStart { .. }
                | ServiceRequest::SystemMonitorPoll { .. }
                | ServiceRequest::SystemMonitorStop { .. }
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
                | ServiceRequest::KeyboardProfileInventoryRead { .. }
                | ServiceRequest::KeyboardOnboardProfilesRead { .. }
                | ServiceRequest::HidNodeKeyboardSerialRead { .. }
                | ServiceRequest::HidNodeKeyboardRelayMetadataRead { .. }
                | ServiceRequest::HidNodeKeyboardRelaySerialRead { .. }
                | ServiceRequest::HidNodeMouseCurrentDpi { change: None, .. }
                | ServiceRequest::HidNodeMouseBrightness {
                    submission: razer_device::mouse_brightness::Submission::Read { .. },
                    ..
                }
                | ServiceRequest::DeviceMouseBrightness {
                    submission: razer_device::mouse_brightness::Submission::Read { .. },
                    ..
                }
                | ServiceRequest::DeviceMouseCurrentDpi { change: None, .. }
                | ServiceRequest::HidNodeMouseDynamic { change: None, .. }
                | ServiceRequest::DeviceMouseDynamic { change: None, .. }
                | ServiceRequest::HidNodeMouseRotation { change: None, .. }
                | ServiceRequest::DeviceMouseRotation { change: None, .. }
                | ServiceRequest::BleMouseRotation { change: None, .. }
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
                | ServiceRequest::AudioEndpoints { .. }
                | ServiceRequest::AudioNotificationsEnable { .. }
                | ServiceRequest::AudioNotificationsDrain
                | ServiceRequest::ForegroundMonitorStart { .. }
                | ServiceRequest::ForegroundMonitorStop { .. }
                | ServiceRequest::ForegroundMonitorEvents { .. }
                | ServiceRequest::HidNodeRead { .. }
                | ServiceRequest::HidNodeHanboCoolerRead { .. }
                | ServiceRequest::HidNodeHanboStartupRead { .. }
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
        let marie_timeout = match &request {
            ServiceRequest::SeirenNativeOpen { .. } => {
                Some(razer_device::seiren_eq::NativeOperation::DeviceInitialize.service_budget())
            }
            ServiceRequest::SeirenNativeCall {
                native_operation, ..
            } => Some(native_operation.service_budget()),
            ServiceRequest::SeirenNativeClose { .. }
            | ServiceRequest::SeirenNativeEvents { .. } => Some(Duration::from_secs(20)),
            _ => None,
        };
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
        let timeout = if reading_keyboard_inventory {
            Duration::from_secs(125)
        } else if philips_measurement || (shutting_down && self.philips_measurement_started) {
            // Measurement registration/release may combine native callbacks
            // with several bounded 20-second Feature transactions. Cancellation
            // is its own protocol command and must finish restoration first.
            Duration::from_secs(180)
        } else if shutting_down {
            // Recorder/mapping restoration, monitor thread completion and its
            // independent launcher callbacks precede the engine shutdowns.
            Duration::from_secs(45)
        } else if let Some(budget) = marie_timeout {
            // Endpoint reconstruction waits 60/30 seconds in the verified
            // mode/sample-rate chain. Do not kill the session before its own
            // bounded operation and cleanup can report their actual result.
            budget + Duration::from_secs(5)
        } else if controller_initialization {
            Duration::from_secs(305)
        } else if controller_triggers {
            // Eight current getters or four ordered setters retain their source
            // retry budgets. Retained receiver lighting can wait for its first
            // real frame and independent native/Feature cleanup. Scope
            // cancellation still interrupts every 50ms.
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
        } else if publishing_shortcuts {
            // Four dependency documents and the referencing shortcut document
            // each retain their own bounded native storage callback.
            Duration::from_secs(25)
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
                if self.request_scope_canceled() {
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
        if !shutting_down && self.request_scope_canceled() {
            return Err(self.stop_after_error(anyhow::anyhow!(
                "设备操作范围已取消；旧响应未提交，已发送的设置仍可能生效"
            )));
        }
        if response.data.is_some() == response.error.is_some() {
            return Err(self.stop_after_error(anyhow::anyhow!("设备服务响应状态无效，连接已关闭")));
        }
        let confirmation = shutting_down.then(|| worker_shutdown::confirm(&response, id));
        if let Some(Ok(confirmation)) = &confirmation {
            self.shutdown_confirmation = Some(*confirmation);
        }
        let stop = if response.fatal || shutting_down {
            self.finish_process()
        } else {
            Ok(())
        };
        if let Some(error) = response.error {
            return Err(self
                .diagnostics
                .attach(with_cleanup_error(anyhow::Error::msg(error), stop)));
        }
        if let Some(Err(error)) = confirmation {
            return Err(self.diagnostics.attach(with_cleanup_error(error, stop)));
        }
        stop.map_err(|error| self.diagnostics.attach(error))?;
        let data = response.data.context("设备服务返回了空响应")?;
        Ok(data)
    }

    fn stop_after_error(&mut self, error: anyhow::Error) -> anyhow::Error {
        let cleanup = self.stop();
        self.diagnostics.attach(with_cleanup_error(error, cleanup))
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        let request_shutdown = !self.stopped
            && self.child.is_some()
            && self.input_worker.is_some()
            && self.output_worker.is_some();
        self.stopped = true;
        let cleanup = if request_shutdown {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            let result = (|| {
                let mut frame = serde_json::to_vec(&RequestEnvelope {
                    id,
                    request: ServiceRequest::Shutdown,
                })?;
                frame.push(b'\n');
                self.requests
                    .as_ref()
                    .context("设备服务清理管道不可用")?
                    .send(frame)?;
                // The longest admitted operation has a 180-second budget;
                // teardown retains its separate 45-second cleanup allowance.
                worker_shutdown::drain(&self.responses, id, Duration::from_secs(225))
            })();
            // Closing stdin also enters explicit server teardown if its reply
            // channel failed. Preserve that failure even when the process exits.
            self.requests.take();
            if result.is_err() {
                if let Some(child) = self.child.as_mut() {
                    let deadline = std::time::Instant::now() + Duration::from_secs(45);
                    while matches!(child.try_wait(), Ok(None))
                        && std::time::Instant::now() < deadline
                    {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
            result.map(|confirmation| {
                self.shutdown_confirmation = Some(confirmation);
            })
        } else {
            Ok(())
        };
        let process = self.finish_process();
        combine_cleanup([cleanup, process])
    }

    fn finish_process(&mut self) -> anyhow::Result<()> {
        self.requests.take();
        self.stopped = true;
        // A shutdown reply is emitted only after owner teardown. Closing the
        // job is a final process boundary, never proof of successful restoration.
        #[cfg(windows)]
        self.job.take();
        // If the OS cannot establish process exit, keep the process and pipe
        // handles with this owner so another cleanup attempt can inspect them.
        reap_child(&mut self.child)?;
        // Terminating the process unblocks pipe reads/writes. Every handle stays
        // owned through its join, including partial startup and request failure.
        let input = join_pipe(self.input_worker.take(), "输入");
        let output = join_pipe(self.output_worker.take(), "输出");
        let diagnostics = join_pipe(self.diagnostic_worker.take(), "诊断");
        combine_cleanup([input, output, diagnostics])
    }
}

fn forward_requests(mut writer: impl Write, pending: Receiver<Vec<u8>>) -> anyhow::Result<()> {
    while let Ok(frame) = pending.recv() {
        writer
            .write_all(&frame)
            .and_then(|_| writer.flush())
            .context("写入设备服务请求失败")?;
    }
    Ok(())
}

fn join_pipe(worker: Option<JoinHandle<anyhow::Result<()>>>, name: &str) -> anyhow::Result<()> {
    match worker {
        Some(worker) => worker
            .join()
            .map_err(|_| anyhow::anyhow!("设备服务{name}线程异常退出"))?,
        None => Ok(()),
    }
}

fn combine_cleanup<const N: usize>(results: [anyhow::Result<()>; N]) -> anyhow::Result<()> {
    let errors = results
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("{}", errors.join("；"))
    }
}

/// Unprefixed vendor diagnostics may use a legacy code page. Once the worker
/// emits our prefix, however, every byte belongs to the owned IPC protocol:
/// decoding or framing failure is terminal, never a log line to skip.
fn forward_responses(
    reader: &mut impl BufRead,
    sender: Sender<anyhow::Result<ResponseEnvelope>>,
    diagnostics: &worker_diagnostics::WorkerDiagnostics,
) -> anyhow::Result<()> {
    enum Frame {
        Diagnostic,
        Response(ResponseEnvelope),
        Closed,
    }
    loop {
        let response = (|| {
            let Some(line) = read_frame(reader).context("读取设备服务响应失败，连接已关闭")?
            else {
                return Ok(Frame::Closed);
            };
            let Some(json) = line.strip_prefix(FRAME_PREFIX.as_bytes()) else {
                diagnostics.record_stdout(&line)?;
                return Ok(Frame::Diagnostic);
            };
            let response = serde_json::from_slice::<ResponseEnvelope>(json)
                .context("设备服务响应格式无效，连接已关闭")?;
            Ok(Frame::Response(response))
        })();
        match response {
            Ok(Frame::Closed) => {
                let _ = sender.send(Err(anyhow::anyhow!("设备服务输出已关闭")));
                return Ok(());
            }
            Ok(Frame::Diagnostic) => continue,
            Ok(Frame::Response(response)) => {
                if sender.send(Ok(response)).is_err() {
                    return Ok(());
                }
            }
            Err(error) => {
                let failure = anyhow::anyhow!("{error:#}");
                let _ = sender.send(Err(error));
                return Err(failure);
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
        if let Err(error) = self.stop() {
            eprintln!("[device service cleanup] {error:#}");
        }
    }
}

/// A test seam for owned-process cleanup; tests never launch a helper.
trait ChildProcess {
    fn exited(&mut self) -> std::io::Result<bool>;
    fn terminate(&mut self) -> std::io::Result<()>;
    fn wait_for_exit(&mut self) -> std::io::Result<()>;
}

impl ChildProcess for Child {
    fn exited(&mut self) -> std::io::Result<bool> {
        self.try_wait().map(|status| status.is_some())
    }

    fn terminate(&mut self) -> std::io::Result<()> {
        self.kill()
    }

    fn wait_for_exit(&mut self) -> std::io::Result<()> {
        self.wait().map(|_| ())
    }
}

// This wait belongs to the same background owner as the request and pipe joins.
// A successful kill alone does not prove that the child has exited or been reaped.
fn terminate_child(child: &mut impl ChildProcess) -> anyhow::Result<()> {
    let observed = child.exited();
    if matches!(observed, Ok(true)) {
        return Ok(());
    }
    if let Err(error) = child.terminate() {
        // A failed kill may mean the child exited between the two operations.
        // If it is still running or the OS cannot tell, retain ownership and
        // return the failure instead of waiting indefinitely on a live process.
        let raced = child.exited();
        if matches!(raced, Ok(true)) {
            return Ok(());
        }
        return combine_cleanup([
            observed.map(|_| ()).context("查询设备服务子进程状态失败"),
            Err(anyhow::Error::new(error).context("无法结束设备服务子进程")),
            raced
                .map(|_| ())
                .context("结束失败后查询设备服务子进程状态失败"),
        ]);
    }
    // A successful termination request still needs a completed wait before any
    // pipe-thread joins or owner destruction can report completion.
    match child.wait_for_exit() {
        Ok(()) => Ok(()),
        Err(error) => {
            let mut cleanup = anyhow::Error::new(error).context("等待设备服务子进程退出失败");
            if let Err(error) = observed {
                cleanup = cleanup.context(format!("查询设备服务子进程状态失败：{error}"));
            }
            Err(cleanup)
        }
    }
}

fn reap_child(child: &mut Option<impl ChildProcess>) -> anyhow::Result<()> {
    if let Some(child) = child.as_mut() {
        terminate_child(child)?;
    }
    child.take();
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
            // prefixed JSON response needs UTF-8; diagnostics retain raw bytes.
            return Ok(Some(bytes));
        }
    }
}

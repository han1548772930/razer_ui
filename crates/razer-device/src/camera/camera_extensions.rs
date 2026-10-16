//! Current PID 3594 private UVC controls, independently of the COM adapter.
//!
//! The SHA-pinned Pro class uses topology node 3 and an eight-byte ISP command
//! through IKsControl::KsProperty (SET, 10 ms, GET). These are native wire
//! requests, not calls into a vendor DLL. IDA/Hex-Rays evidence and the current
//! Complete native bodies and class dispatch are retained in the analysis
//! corpus. `camera-extension-export-semantics.json` records supplemental
//! native exports independently of their current JavaScript/UI callers.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::camera::KIYO_V2_PRO_PRODUCT_ID;

pub const CAMERA_EXTENSION_NODE: u32 = 3;
pub const CAMERA_EXTENSION_PROPERTY: u32 = 1;
pub const CAMERA_EXTENSION_SET_FLAGS: u32 = 0x1000_0002;
pub const CAMERA_EXTENSION_GET_FLAGS: u32 = 0x1000_0001;
pub const CAMERA_EXTENSION_DELAY_MS: u32 = 10;
pub const CAMERA_EXTENSION_PROPERTY_SET: [u8; 16] = *b"RAZER_USBCAM_EXT";

/// Entries in the native vector initialized at RVA 0x1020. Index is the wire
/// code; values are the source's offset representation, not signed EV stops.
pub const EV_COMPENSATION_VALUES: [u8; 19] = [
    60, 57, 53, 50, 47, 43, 40, 37, 33, 30, 27, 23, 20, 17, 13, 10, 7, 3, 0,
];

/// The current 3594 factory actions and independently audited native exports.
/// Protocol support does not establish a source UI binding. The current SetLDC
/// caller always supplies mode 0; save-flash's boolean argument is ignored by
/// the native method. LONG/int values retain the native low-byte conversion.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraExtensionRequest {
    SetHdr {
        enabled: bool,
    },
    GetUvcExtCmdStatus,
    SetLdc {
        enabled: bool,
    },
    SetAutoFocusMode {
        mode: i32,
    },
    SetHdrBrightness {
        brightness: i32,
    },
    SetHdrMode {
        mode: i32,
    },
    SetMicNoiseSuppression {
        enabled: bool,
    },
    SaveUvcExtCmdToFlash,
    GetWatermark,
    SetWatermark {
        state: i32,
        position: i32,
    },
    GetIsoSensitivity,
    SetIsoSensitivity {
        code: i32,
    },
    GetFdafMode,
    SetFdafMode {
        mode: i32,
        light_condition: i32,
    },
    SetAutoFramingSetting {
        command: i32,
        position: i32,
    },
    GetDynamicNoiseReduction {
        channel: i32,
    },
    SetDynamicNoiseReduction {
        channel: i32,
        enabled: bool,
    },
    GetMirrorVideo,
    SetMirrorVideo {
        mode: i32,
    },
    GetAutoExposureMetering,
    SetAutoExposureMetering {
        mode: i32,
    },
    GetShutterSpeed,
    SetShutterSpeed {
        value: i32,
    },
    GetShutterLimitControl,
    SetShutterLimitControl {
        mode: i32,
    },
    GetEvCompensation,
    SetEvCompensation {
        value: i32,
    },
    SetAutoFocusLowSensitivity {
        value: i32,
    },
    RestoreDefaults,
    RebootDevice,
    RestoreDefaultAndReboot,
    GetAdaptiveMjpegQuality {
        query: i32,
    },
    SetAdaptiveMjpegQuality {
        enabled: bool,
        quality: i32,
    },
    GetFixedMjpegQuality,
    SetFixedMjpegQuality {
        quality: i32,
    },
    GetStreamingStatus,
    SetAutoframing {
        mode: i32,
    },
    SetAudioFunction {
        adaptive_noise_reduction: i32,
        beamforming: i32,
    },
    GetAudioFunction,
    SetAutoframingSpeedAndSize {
        speed: i32,
        size: i32,
    },
    GetAutoframingSpeedAndSize,
}

impl CameraExtensionRequest {
    pub const fn action(self) -> &'static str {
        match self {
            Self::SetHdr { .. } => "SetHDR",
            Self::GetUvcExtCmdStatus => "GetUVCExtCmdStatus",
            Self::SetLdc { .. } => "SetLDC",
            Self::SetAutoFocusMode { .. } => "SetAutoFocusMode",
            Self::SetHdrBrightness { .. } => "SetHDRBrightness",
            Self::SetHdrMode { .. } => "SetHDRMode",
            Self::SetMicNoiseSuppression { .. } => "SetMicNoiseSuppression",
            Self::SaveUvcExtCmdToFlash => "SaveUVCExtCmdToFlash",
            Self::GetWatermark => "GetWatermark",
            Self::SetWatermark { .. } => "SetWatermark",
            Self::GetIsoSensitivity => "GetISOSensitivity",
            Self::SetIsoSensitivity { .. } => "SetISOSensitivity",
            Self::GetFdafMode => "GetFDAFMode",
            Self::SetFdafMode { .. } => "SetFDAFMode",
            Self::SetAutoFramingSetting { .. } => "SetAutoFramingSetting",
            Self::GetDynamicNoiseReduction { .. } => "GetDynamicNoiseReduction",
            Self::SetDynamicNoiseReduction { .. } => "SetDynamicNoiseReduction",
            Self::GetMirrorVideo => "GetMirrorVideo",
            Self::SetMirrorVideo { .. } => "SetMirrorVideo",
            Self::GetAutoExposureMetering => "GetAutoExposureMetering",
            Self::SetAutoExposureMetering { .. } => "SetAutoExposureMetering",
            Self::GetShutterSpeed => "GetShutterSpeed",
            Self::SetShutterSpeed { .. } => "SetShutterSpeed",
            Self::GetShutterLimitControl => "GetShutterLimitControl",
            Self::SetShutterLimitControl { .. } => "SetShutterLimitControl",
            Self::GetEvCompensation => "GetEVCompensation",
            Self::SetEvCompensation { .. } => "SetEVCompensation",
            Self::SetAutoFocusLowSensitivity { .. } => "SetAutoFocusLowSensitivity",
            Self::RestoreDefaults => "RestoreDefaults",
            Self::RebootDevice => "RebootDevice",
            Self::RestoreDefaultAndReboot => "RestoreDefaultAndReboot",
            Self::GetAdaptiveMjpegQuality { .. } => "GetAdaptiveMJPEGQuality",
            Self::SetAdaptiveMjpegQuality { .. } => "SetAdaptiveMJPEGQuality",
            Self::GetFixedMjpegQuality => "GetFixedMJPEGQuality",
            Self::SetFixedMjpegQuality { .. } => "SetFixedMJPEGQuality",
            Self::GetStreamingStatus => "GetStreamingStatus",
            Self::SetAutoframing { .. } => "SetAutoframing",
            Self::SetAudioFunction { .. } => "SetAudioFunction",
            Self::GetAudioFunction => "GetAudioFunction",
            Self::SetAutoframingSpeedAndSize { .. } => "SetAutoframingSpeedAndSize",
            Self::GetAutoframingSpeedAndSize => "GetAutoframingSpeedAndSize",
        }
    }

    /// Native getter actions still send a KS SET to select the ISP query.
    /// `mutates_state` identifies actual state writes, not the KS direction.
    pub fn commands(self) -> Vec<CameraExtensionCommand> {
        use CameraExtensionRequest::*;
        let one = |bytes: &[u8], mutates_state: bool, native_rva| {
            vec![CameraExtensionCommand::new(
                bytes,
                mutates_state,
                native_rva,
            )]
        };
        match self {
            SetHdr { enabled } => one(&[0xa0, 0x01, enabled.into()], true, 0x52230),
            GetUvcExtCmdStatus => one(&[0xc0, 0x03, 0x11], false, 0x4d810),
            SetLdc { enabled } => one(&[0xff, 0x01, enabled.into(), 3, 0], true, 0x4d630),
            SetAutoFocusMode { mode } => one(&[0xff, 0x06, mode as u8], true, 0x4d6a0),
            SetHdrBrightness { brightness } => {
                one(&[0xa0, 0x15, 1, brightness as u8], true, 0x52290)
            }
            // Both exports use slot +0x40 in CRzDeviceAccess_0E0A. The Pro
            // override is HDR brightness; the base class's FF 08 command is
            // not the dispatch target for PID 3594.
            SetHdrMode { mode } => one(&[0xa0, 0x15, 1, mode as u8], true, 0x52290),
            SetMicNoiseSuppression { enabled } => one(&[0xff, 0x04, enabled.into()], true, 0x4d760),
            SaveUvcExtCmdToFlash => one(&[0xc0, 0x03, 0xa8], true, 0x4d7c0),
            GetWatermark => one(&[0xc0, 0x08], false, 0x4d940),
            SetWatermark { state, position } => {
                one(&[0xc0, 0x08, 1, state as u8, position as u8], true, 0x4d8b0)
            }
            GetIsoSensitivity => one(&[0xa0, 0x13], false, 0x52af0),
            SetIsoSensitivity { code } => one(&[0xa0, 0x13, 1, code as u8], true, 0x52a70),
            GetFdafMode => one(&[0xc0, 0x0a], false, 0x4db50),
            SetFdafMode {
                mode,
                light_condition,
            } => one(
                &[0xc0, 0x0a, 1, mode as u8, 0, 0, 0, light_condition as u8],
                true,
                0x4dac0,
            ),
            SetAutoFramingSetting { command, position } => {
                one(&[0xff, 0x14, command as u8, position as u8], true, 0x4dbc0)
            }
            GetDynamicNoiseReduction { .. } => one(&[0xa0, 0x06, 2], false, 0x52370),
            SetDynamicNoiseReduction { enabled, .. } => {
                one(&[0xa0, 0x06, enabled.into()], true, 0x522f0)
            }
            GetMirrorVideo => one(&[0xa0, 0x07, 2], false, 0x52460),
            SetMirrorVideo { mode } => one(&[0xa0, 0x07, mode as u8], true, 0x523e0),
            GetAutoExposureMetering => one(&[0xa0, 0x08, 0x0f], false, 0x52550),
            SetAutoExposureMetering { mode } => one(&[0xa0, 0x08, mode as u8], true, 0x524d0),
            GetShutterSpeed => one(&[0xa0, 0x09], false, 0x52680),
            SetShutterSpeed { value } => {
                let value = value.to_be_bytes();
                one(
                    &[0xa0, 0x09, 1, value[0], value[1], value[2], value[3]],
                    true,
                    0x525c0,
                )
            }
            GetShutterLimitControl => one(&[0xa0, 0x1c], false, 0x52790),
            SetShutterLimitControl { mode } => one(&[0xa0, 0x1c, 1, mode as u8], true, 0x52710),
            GetEvCompensation => one(&[0xa0, 0x14], false, 0x52d60),
            SetEvCompensation { value } => one(
                &[0xa0, 0x14, 1, nearest_ev_code(value as u8)],
                true,
                0x52c50,
            ),
            SetAutoFocusLowSensitivity { value } => one(&[0xa0, 0x18, value as u8], true, 0x52800),
            RestoreDefaults => one(&[0xa0, 0x0a, 2], true, 0x52860),
            RebootDevice => one(&[0xc0, 0x0e, 7, 2], true, 0x4e700),
            RestoreDefaultAndReboot => one(&[0xc0, 0x0e, 7, 1], true, 0x4e700),
            GetAdaptiveMjpegQuality { query } => {
                one(&[0xc0, 0x0e, 0x0a, query as u8], false, 0x4e1c0)
            }
            SetAdaptiveMjpegQuality { enabled, quality } => one(
                &[0xc0, 0x0e, 9, enabled.into(), 0, 0, quality as u8],
                true,
                0x4e240,
            ),
            GetFixedMjpegQuality => one(&[0xa0, 0x0b], false, 0x528b0),
            SetFixedMjpegQuality { quality } => one(&[0xa0, 0x0b, 1, quality as u8], true, 0x52920),
            GetStreamingStatus => one(&[0xc0, 9, 0x0a], false, 0x4e3c0),
            SetAutoframing { mode } => one(&[0xa0, 0x0c, mode as u8], true, 0x529a0),
            SetAudioFunction {
                adaptive_noise_reduction,
                beamforming,
            } => {
                let primary = CameraExtensionCommand::new(
                    &[
                        0xc0,
                        0x0e,
                        0xa0,
                        adaptive_noise_reduction as u8,
                        beamforming as u8,
                    ],
                    true,
                    0x4e500,
                );
                let mut persist = CameraExtensionCommand::new(&[0xc0, 3, 0xa8], true, 0x4d7c0);
                persist.wait_before_ms = CAMERA_EXTENSION_DELAY_MS;
                let mut reboot = CameraExtensionCommand::new(&[0xc0, 0x0e, 7, 2], true, 0x4e700);
                reboot.wait_before_ms = CAMERA_EXTENSION_DELAY_MS;
                vec![primary, persist, reboot]
            }
            GetAudioFunction => one(&[0xc0, 0x0e, 0xa1], false, 0x4e5b0),
            SetAutoframingSpeedAndSize { speed, size } => vec![
                CameraExtensionCommand::new(&[0xa0, 0x17, speed as u8], true, 0x52de0),
                CameraExtensionCommand::new(&[0xa0, 0x16, size as u8], true, 0x52de0),
            ],
            GetAutoframingSpeedAndSize => vec![
                CameraExtensionCommand::new(&[0xa0, 0x17, 3], false, 0x52e90),
                CameraExtensionCommand::new(&[0xa0, 0x16, 3], false, 0x52e90),
            ],
        }
    }
}

fn nearest_ev_code(value: u8) -> u8 {
    // min_by_key keeps the first equal-distance entry, matching native `<`.
    EV_COMPENSATION_VALUES
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| value.abs_diff(**candidate))
        .map(|(code, _)| code as u8)
        .expect("the native EV table is nonempty")
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraExtensionCommand {
    pub bytes: [u8; 8],
    pub mutates_state: bool,
    pub native_rva: u32,
    pub wait_before_ms: u32,
}

impl CameraExtensionCommand {
    fn new(prefix: &[u8], mutates_state: bool, native_rva: u32) -> Self {
        let mut bytes = [0; 8];
        bytes[..prefix.len()].copy_from_slice(prefix);
        Self {
            bytes,
            mutates_state,
            native_rva,
            wait_before_ms: 0,
        }
    }
}

/// Explicit KSP_NODE serialization also initializes the reserved DWORD that
/// the native implementation leaves uninitialized. GUID is already LE bytes.
pub fn camera_extension_header(is_set: bool) -> [u8; 32] {
    let mut header = [0; 32];
    header[..16].copy_from_slice(&CAMERA_EXTENSION_PROPERTY_SET);
    header[16..20].copy_from_slice(&CAMERA_EXTENSION_PROPERTY.to_le_bytes());
    let flags = if is_set {
        CAMERA_EXTENSION_SET_FLAGS
    } else {
        CAMERA_EXTENSION_GET_FLAGS
    };
    header[20..24].copy_from_slice(&flags.to_le_bytes());
    header[24..28].copy_from_slice(&CAMERA_EXTENSION_NODE.to_le_bytes());
    header
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraKsCallOutcome {
    pub hresult: i32,
    pub bytes_returned: u32,
}

/// Begin queries IKsTopologyInfo then creates a node-3 IKsControl instance.
/// `end_node` must release all temporary COM references, including partial
/// open failures. The executor invokes it after every begin attempt.
pub trait CameraExtensionTransport {
    fn begin_node(&mut self, node_id: u32) -> Result<()>;
    fn ks_property(&mut self, header: &[u8; 32], data: &mut [u8; 8])
    -> Result<CameraKsCallOutcome>;
    fn end_node(&mut self);
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        std::thread::sleep(std::time::Duration::from_millis(milliseconds.into()));
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraUvcStatus {
    pub ldc: u8,
    pub hdr: u8,
    pub hdr_mode: u8,
    pub autofocus_mode: u8,
    pub adaptive_noise_reduction: u8,
}

impl CameraUvcStatus {
    pub const fn hdr_enabled(self) -> bool {
        self.hdr == 1
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraExtensionValue {
    /// A setter ACK does not independently verify the requested state.
    Acknowledged,
    UvcStatus(CameraUvcStatus),
    Watermark {
        state: u8,
        position: u8,
    },
    IsoSensitivity {
        raw_code: u8,
        iso: Option<u32>,
    },
    FdafMode {
        mode: u8,
        light_condition: u8,
    },
    Byte(u8),
    ShutterSpeed(i32),
    EvCompensation {
        raw_code: u8,
        value: Option<u8>,
    },
    StreamingStatus {
        raw_code: u8,
        is_streaming: bool,
    },
    AudioFunction {
        adaptive_noise_reduction: u8,
        beamforming: u8,
    },
    AutoframingSpeedAndSize {
        speed: u8,
        size: u8,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraExtensionPhase {
    Target,
    Validation,
    OpenControl,
    Set,
    Wait,
    Get,
    Decode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraExtensionFailure {
    pub phase: CameraExtensionPhase,
    pub command_index: usize,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraExtensionExchange {
    pub command: CameraExtensionCommand,
    pub set_attempted: bool,
    pub set_outcome: Option<CameraKsCallOutcome>,
    pub get_attempted: bool,
    pub get_outcome: Option<CameraKsCallOutcome>,
    pub response: Option<[u8; 8]>,
    pub acknowledged: bool,
}

impl CameraExtensionExchange {
    fn new(command: CameraExtensionCommand) -> Self {
        Self {
            command,
            set_attempted: false,
            set_outcome: None,
            get_attempted: false,
            get_outcome: None,
            response: None,
            acknowledged: false,
        }
    }
}

/// Failures retain every attempted write and acknowledged phase. Audio writes
/// can have applied before persistence/reboot fails; no rollback is invented.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraExtensionProgress {
    pub request: CameraExtensionRequest,
    pub exchanges: Vec<CameraExtensionExchange>,
    pub value: Option<CameraExtensionValue>,
    pub completed: bool,
    pub failure: Option<CameraExtensionFailure>,
}

impl CameraExtensionProgress {
    pub fn failed(
        request: CameraExtensionRequest,
        phase: CameraExtensionPhase,
        message: String,
    ) -> Self {
        Self {
            request,
            exchanges: Vec::new(),
            value: None,
            completed: false,
            failure: Some(CameraExtensionFailure {
                phase,
                command_index: 0,
                message,
            }),
        }
    }
}

fn run_exchange(
    transport: &mut impl CameraExtensionTransport,
    exchange: &mut CameraExtensionExchange,
    validate: &impl Fn() -> Result<()>,
) -> std::result::Result<(), (CameraExtensionPhase, String)> {
    use CameraExtensionPhase::*;
    let problem = |phase, error: anyhow::Error| (phase, format!("{error:#}"));
    let begin = transport.begin_node(CAMERA_EXTENSION_NODE);
    if let Err(error) = begin {
        transport.end_node();
        return Err(problem(OpenControl, error));
    }
    // This closure always exits before end_node, including transport failures
    // and cancellation between SET and GET.
    let result = (|| {
        validate().map_err(|error| problem(Validation, error))?;
        let mut command = exchange.command.bytes;
        exchange.set_attempted = true;
        let set = transport
            .ks_property(&camera_extension_header(true), &mut command)
            .map_err(|error| problem(Set, error))?;
        exchange.set_outcome = Some(set);
        // Native ignores this HRESULT. Rejecting a failed SET avoids treating
        // a later stale GET as a successful state change; keep the raw result.
        if set.hresult < 0 {
            return Err((
                Set,
                format!("KsProperty SET failed: 0x{:08x}", set.hresult as u32),
            ));
        }
        validate().map_err(|error| problem(Validation, error))?;
        transport
            .wait_ms(CAMERA_EXTENSION_DELAY_MS)
            .map_err(|error| problem(Wait, error))?;
        validate().map_err(|error| problem(Validation, error))?;
        let mut response = [0; 8];
        response[0] = 0xff; // native also seeds failure status before GET
        exchange.get_attempted = true;
        let get = transport.ks_property(&camera_extension_header(false), &mut response);
        exchange.response = Some(response);
        let get = get.map_err(|error| problem(Get, error))?;
        exchange.get_outcome = Some(get);
        if get.hresult != 0 {
            return Err((
                Get,
                format!("KsProperty GET requires S_OK: 0x{:08x}", get.hresult as u32),
            ));
        }
        if get.bytes_returned != 8 {
            return Err((
                Get,
                format!(
                    "KsProperty GET returned {} bytes; expected 8",
                    get.bytes_returned
                ),
            ));
        }
        if response[0] != 0 {
            let description = match response[0] {
                1 => "parameter error",
                2 => "command error",
                _ => "device error",
            };
            return Err((
                Decode,
                format!("private camera {description}: status {}", response[0]),
            ));
        }
        exchange.acknowledged = true;
        Ok(())
    })();
    transport.end_node();
    result
}

pub fn execute_camera_extension(
    transport: &mut impl CameraExtensionTransport,
    product_id: u32,
    request: CameraExtensionRequest,
    validate: &impl Fn() -> Result<()>,
) -> CameraExtensionProgress {
    use CameraExtensionPhase::*;
    if product_id != KIYO_V2_PRO_PRODUCT_ID {
        return CameraExtensionProgress::failed(
            request,
            Target,
            "camera product has no current private UVC protocol audit".into(),
        );
    }
    let mut progress = CameraExtensionProgress {
        request,
        exchanges: Vec::new(),
        value: None,
        completed: false,
        failure: None,
    };
    for (command_index, command) in request.commands().into_iter().enumerate() {
        let failure = (|| {
            validate().map_err(|error| (Validation, format!("{error:#}")))?;
            if command.wait_before_ms != 0 {
                transport
                    .wait_ms(command.wait_before_ms)
                    .map_err(|error| (Wait, format!("{error:#}")))?;
                validate().map_err(|error| (Validation, format!("{error:#}")))?;
            }
            progress
                .exchanges
                .push(CameraExtensionExchange::new(command));
            run_exchange(transport, progress.exchanges.last_mut().unwrap(), validate)?;
            validate().map_err(|error| (Validation, format!("{error:#}")))?;
            Ok::<_, (CameraExtensionPhase, String)>(())
        })();
        if let Err((phase, message)) = failure {
            progress.failure = Some(CameraExtensionFailure {
                phase,
                command_index,
                message,
            });
            return progress;
        }
    }
    progress.value = Some(decode_value(request, &progress.exchanges));
    progress.completed = true;
    progress
}

fn decode_value(
    request: CameraExtensionRequest,
    exchanges: &[CameraExtensionExchange],
) -> CameraExtensionValue {
    use CameraExtensionRequest::*;
    use CameraExtensionValue as Value;
    let response = exchanges[0]
        .response
        .expect("acknowledged exchange has a response");
    match request {
        GetUvcExtCmdStatus => Value::UvcStatus(CameraUvcStatus {
            ldc: response[2],
            hdr: response[3],
            hdr_mode: response[4],
            autofocus_mode: response[5],
            adaptive_noise_reduction: response[6],
        }),
        GetWatermark => Value::Watermark {
            state: response[2],
            position: response[3],
        },
        GetIsoSensitivity => Value::IsoSensitivity {
            raw_code: response[3],
            iso: match response[3] {
                1..=7 => Some(100u32 << (response[3] - 1)),
                _ => None,
            },
        },
        GetFdafMode => Value::FdafMode {
            mode: response[2],
            light_condition: response[7],
        },
        GetShutterSpeed => {
            Value::ShutterSpeed(i32::from_be_bytes(response[3..7].try_into().unwrap()))
        }
        GetEvCompensation => Value::EvCompensation {
            raw_code: response[3],
            value: EV_COMPENSATION_VALUES.get(response[3] as usize).copied(),
        },
        GetStreamingStatus => Value::StreamingStatus {
            raw_code: response[3],
            is_streaming: response[3] == 1,
        },
        GetAudioFunction => Value::AudioFunction {
            adaptive_noise_reduction: response[3],
            beamforming: response[4],
        },
        GetAutoframingSpeedAndSize => Value::AutoframingSpeedAndSize {
            speed: response[3],
            size: exchanges[1].response.unwrap()[3],
        },
        GetDynamicNoiseReduction { .. }
        | GetMirrorVideo
        | GetAutoExposureMetering
        | GetShutterLimitControl
        | GetAdaptiveMjpegQuality { .. }
        | GetFixedMjpegQuality => Value::Byte(response[3]),
        _ => Value::Acknowledged,
    }
}

#[cfg(test)]
#[path = "camera_extensions_tests.rs"]
mod tests;

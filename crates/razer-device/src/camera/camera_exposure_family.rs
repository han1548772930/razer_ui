//! Current GenericCamera 1.0.35.0 base (0E0B) and X (0E0C) exposure.
//! The two vtables use different commands over the same topology-scanning ISP
//! lifecycle as family focus. Pro (0E0A) has its own audited implementation.
//! Setter output is an acknowledgement, never an independent observation.

pub use crate::camera_focus_family::{
    CameraFamilyFocusAttempt as CameraFamilyExposureAttempt,
    CameraFamilyFocusFailure as CameraFamilyExposureFailure,
    CameraFamilyFocusPhase as CameraFamilyExposurePhase,
    CameraFamilyFocusTransport as CameraFamilyExposureTransport,
    camera_family_focus_header as camera_family_exposure_header,
};
use crate::camera_focus_family::{execute_camera_family_isp, validate_camera_family_isp_progress};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraFamilyExposureRequest {
    GetIsoSensitivity,
    SetIsoSensitivity { code: i32 },
    GetAutoExposureMetering,
    SetAutoExposureMetering { mode: i32 },
    GetShutterSpeed,
    SetShutterSpeed { value: i32 },
    GetEvCompensation,
    SetEvCompensation { value: i32 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyExposureCommand {
    pub bytes: [u8; 8],
    pub mutates_state: bool,
    pub native_rva: u32,
}
impl CameraFamilyExposureRequest {
    pub const fn action(self) -> &'static str {
        match self {
            Self::GetIsoSensitivity => "GetISOSensitivity",
            Self::SetIsoSensitivity { .. } => "SetISOSensitivity",
            Self::GetAutoExposureMetering => "GetAutoExposureMetering",
            Self::SetAutoExposureMetering { .. } => "SetAutoExposureMetering",
            Self::GetShutterSpeed => "GetShutterSpeed",
            Self::SetShutterSpeed { .. } => "SetShutterSpeed",
            Self::GetEvCompensation => "GetEVCompensation",
            Self::SetEvCompensation { .. } => "SetEVCompensation",
        }
    }

    /// Native LONG arguments retain their low byte or complete BE32 bits.
    /// X ISO slots point to RVA 0x1ff40: `return 1` without IO or out writes.
    /// Reject that known no-op instead of manufacturing a read/write receipt.
    pub fn command(self, product_id: u32) -> Result<CameraFamilyExposureCommand> {
        use CameraFamilyExposureRequest::*;
        ensure!(
            matches!(product_id, 3595 | 3596),
            "camera family exposure product has no current private audit"
        );
        if product_id == 3596 && matches!(self, GetIsoSensitivity | SetIsoSensitivity { .. }) {
            bail!("PID 3596 ISO is a vendor no-op (RVA 0x1ff40); no device read/write exists");
        }
        let (bytes, mutates_state, native_rva) = match self {
            GetIsoSensitivity => ([0xc0, 9, 8, 0, 0, 0, 0, 0], false, 0x4d990),
            SetIsoSensitivity { code } => ([0xc0, 9, 1, code as u8, 0, 0, 0, 0], true, 0x4d910),
            // Preserve base's actual 0x10 query operand; X uses a different
            // command and returns its raw mode without reverse mapping.
            GetAutoExposureMetering if product_id == 3595 => {
                ([0xc0, 0x0e, 4, 0x10, 0, 0, 0, 0], false, 0x4de10)
            }
            GetAutoExposureMetering => ([5, 0, 0, 0, 0, 0, 0, 0], false, 0x54760),
            SetAutoExposureMetering { mode } if product_id == 3595 => {
                ([0xc0, 0x0e, 4, mode as u8, 0, 0, 0, 0], true, 0x4dd90)
            }
            SetAutoExposureMetering { mode } => {
                // The public export narrows before the X override compares.
                let mode = match mode as u8 {
                    0 => 2,
                    1 => 1,
                    5 => 3,
                    other => {
                        bail!("PID 3596 metering rejects native mode {other}; expected 0, 1 or 5")
                    }
                };
                ([0x25, 0, mode, 0, 0, 0, 0, 0], true, 0x54510)
            }
            GetShutterSpeed if product_id == 3595 => ([0xc0, 9, 6, 0, 0, 0, 0, 0], false, 0x4df40),
            GetShutterSpeed => ([7, 0, 0, 0, 0, 0, 0, 0], false, 0x54a40),
            SetShutterSpeed { value } if product_id == 3595 => {
                let b = value.to_be_bytes();
                ([0xc0, 9, 5, b[0], b[1], b[2], b[3], 0], true, 0x4de80)
            }
            SetShutterSpeed { value } => ([0x27, 0, value as u8, 0, 0, 0, 0, 0], true, 0x548d0),
            // X inherits the base EV methods, but dispatches ISP through its
            // own GUID/property-5 transport. Pro's EV table does not apply.
            GetEvCompensation => ([0xc0, 0x0e, 6, 0, 0, 0, 0, 0], false, 0x4e050),
            SetEvCompensation { value } => {
                ([0xc0, 0x0e, 5, value as u8, 0, 0, 0, 0], true, 0x4dfd0)
            }
        };
        Ok(CameraFamilyExposureCommand {
            bytes,
            mutates_state,
            native_rva,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CameraFamilyExposureValue {
    /// The original setter's out value. It confirms the accepted exchange,
    /// and is not an independently queried device state.
    Acknowledged {
        native_value: i32,
    },
    /// Base returns an actual BE32 ISO number, not Pro's selector code.
    IsoSensitivity {
        iso: u32,
    },
    AutoExposureMetering {
        mode: i32,
    },
    ShutterSpeed {
        value: i32,
    },
    EvCompensation {
        value: i32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CameraFamilyExposureProgress {
    pub product_id: u32,
    pub request: CameraFamilyExposureRequest,
    pub command: Option<CameraFamilyExposureCommand>,
    pub topology_nodes: Option<u32>,
    /// Retain every attempted SET, including a write whose GET failed before
    /// a later topology node accepted the command.
    pub attempts: Vec<CameraFamilyExposureAttempt>,
    pub value: Option<CameraFamilyExposureValue>,
    pub completed: bool,
    pub failure: Option<CameraFamilyExposureFailure>,
}
impl CameraFamilyExposureProgress {
    pub fn failed(
        product_id: u32,
        request: CameraFamilyExposureRequest,
        phase: CameraFamilyExposurePhase,
        message: String,
    ) -> Self {
        Self {
            product_id,
            request,
            command: request.command(product_id).ok(),
            topology_nodes: None,
            attempts: vec![],
            value: None,
            completed: false,
            failure: Some(CameraFamilyExposureFailure {
                phase,
                node_id: None,
                message,
            }),
        }
    }
}

fn decode(
    product_id: u32,
    request: CameraFamilyExposureRequest,
    response: [u8; 8],
) -> Result<CameraFamilyExposureValue> {
    use CameraFamilyExposureRequest::*;
    use CameraFamilyExposureValue as V;
    let command = request.command(product_id)?;
    ensure!(
        response[0] == 0,
        "camera exposure device status is not accepted"
    );
    let be32 = || i32::from_be_bytes(response[3..7].try_into().unwrap());
    let ack = |native_value| V::Acknowledged { native_value };
    Ok(match request {
        GetIsoSensitivity => V::IsoSensitivity { iso: be32() as u32 },
        SetIsoSensitivity { .. } => ack(response[2].into()),
        GetAutoExposureMetering => V::AutoExposureMetering {
            mode: response[if product_id == 3595 { 3 } else { 2 }].into(),
        },
        SetAutoExposureMetering { .. } => ack(if product_id == 3595 {
            response[3].into()
        } else {
            command.bytes[2].into()
        }),
        GetShutterSpeed => V::ShutterSpeed {
            value: if product_id == 3595 {
                be32()
            } else {
                response[2].into()
            },
        },
        SetShutterSpeed { value } => ack(if product_id == 3595 { be32() } else { value }),
        GetEvCompensation => V::EvCompensation {
            value: response[3].into(),
        },
        SetEvCompensation { .. } => ack(response[3].into()),
    })
}

pub fn execute_camera_family_exposure(
    transport: &mut impl CameraFamilyExposureTransport,
    product_id: u32,
    request: CameraFamilyExposureRequest,
    validate: &impl Fn() -> Result<()>,
) -> CameraFamilyExposureProgress {
    let command = match request.command(product_id) {
        Ok(command) => command,
        Err(e) => {
            return CameraFamilyExposureProgress::failed(
                product_id,
                request,
                CameraFamilyExposurePhase::Target,
                e.to_string(),
            );
        }
    };
    let exchange = execute_camera_family_isp(transport, product_id, command.bytes, validate);
    // A cancellation after ACK retains both the raw exchange and decoded
    // value. It must not erase an already accepted write or invent rollback.
    let value = exchange
        .attempts
        .last()
        .filter(|a| a.acknowledged)
        .and_then(|a| a.response)
        .map(|response| decode(product_id, request, response).unwrap());
    CameraFamilyExposureProgress {
        product_id,
        request,
        command: Some(command),
        topology_nodes: exchange.topology_nodes,
        attempts: exchange.attempts,
        value,
        completed: exchange.completed,
        failure: exchange.failure,
    }
}

/// Pure receipt verification before IPC/service results are published. No
/// value is accepted unless its exact request has an eight-byte status-0 GET.
pub fn validate_camera_family_exposure_progress(
    product_id: u32,
    request: CameraFamilyExposureRequest,
    progress: &CameraFamilyExposureProgress,
) -> Result<()> {
    ensure!(
        progress.product_id == product_id && progress.request == request,
        "family exposure receipt identity mismatch"
    );
    match request.command(product_id) {
        Ok(command) => ensure!(
            progress.command == Some(command),
            "family exposure packet/native metadata mismatch"
        ),
        Err(_) => ensure!(
            progress.command.is_none()
                && progress
                    .failure
                    .as_ref()
                    .is_some_and(|f| f.phase == CameraFamilyExposurePhase::Target),
            "family exposure unsupported command was not rejected before transport"
        ),
    }
    validate_camera_family_isp_progress(
        progress.topology_nodes,
        &progress.attempts,
        progress.completed,
        progress.failure.as_ref(),
    )?;
    let expected = progress
        .attempts
        .last()
        .filter(|a| a.acknowledged)
        .and_then(|a| a.response)
        .map(|response| decode(product_id, request, response))
        .transpose()?;
    ensure!(
        progress.value == expected,
        "family exposure value is not the decoded acknowledged response"
    );
    Ok(())
}

#[cfg(test)]
#[path = "camera_exposure_family_tests.rs"]
mod tests;

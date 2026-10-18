//! Mapping-engine mouse rotation through the Windows filter driver.
//! IDA 8.3: GET 0x113CA0, SET 0x114130, completion 0x114850.
//! This is a four-byte IOCTL, separate from HID Feature angle tuning.
use crate::mapping_native_output::{DriverOperation, DriverOutputError, SubmissionId};
use std::{error::Error, fmt, time::Duration};

pub const GET_ROTATION_IOCTL: u32 = 0x8888_3184;
pub const SET_ROTATION_IOCTL: u32 = 0x8888_3180;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RotationRequest {
    GetAngle,
    SetAngle(i32),
}

impl RotationRequest {
    pub fn ioctl(self) -> u32 {
        match self {
            Self::GetAngle => GET_ROTATION_IOCTL,
            Self::SetAngle(_) => SET_ROTATION_IOCTL,
        }
    }

    pub fn input(self) -> Option<[u8; 4]> {
        match self {
            Self::GetAngle => None,
            Self::SetAngle(angle) => Some(angle.to_le_bytes()),
        }
    }

    pub fn output_size(self) -> u32 {
        match self {
            Self::GetAngle => 4,
            Self::SetAngle(_) => 0,
        }
    }
}

/// Actual current task's native fallback: disable => zero; enable =>
/// (angle + 360) % 360. The audited editor only permits -44..44.
pub fn native_angle_for_task(enabled: bool, degrees: i8) -> Result<i32, RotationError> {
    if !(-44..=44).contains(&degrees) {
        return Err(RotationError::EditorAngleOutOfRange(degrees));
    }
    Ok(if enabled {
        (i32::from(degrees) + 360) % 360
    } else {
        0
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RotationReply {
    SetComplete,
    Angle { raw: i32, bytes: [u8; 4] },
}

impl RotationReply {
    /// No read buffer is interpreted until the driver reports success and
    /// supplies exactly four bytes. Unknown native angle values stay raw.
    pub fn from_read_bytes(bytes: &[u8]) -> Result<Self, RotationError> {
        let bytes: [u8; 4] = bytes
            .try_into()
            .map_err(|_| RotationError::ReadLength(bytes.len()))?;
        Ok(Self::Angle {
            raw: i32::from_le_bytes(bytes),
            bytes,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RotationError {
    Driver(DriverOutputError),
    EditorAngleOutOfRange(i8),
    ReadLength(usize),
    UnexpectedReply,
    SessionInvalid,
    Canceled,
}

impl From<DriverOutputError> for RotationError {
    fn from(error: DriverOutputError) -> Self {
        Self::Driver(error)
    }
}

impl fmt::Display for RotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => error.fmt(f),
            Self::EditorAngleOutOfRange(angle) => {
                write!(f, "rotation editor angle {angle} is outside -44..44")
            }
            Self::ReadLength(length) => {
                write!(f, "rotation driver returned {length} bytes; expected four")
            }
            Self::UnexpectedReply => f.write_str("rotation completion has the wrong operation"),
            Self::SessionInvalid => f.write_str("retained rotation device session is invalid"),
            Self::Canceled => f.write_str("rotation operation was canceled"),
        }
    }
}

impl Error for RotationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RotationPoll {
    Pending,
    Completed(Result<RotationReply, RotationError>),
}

/// Implementations retain buffers until final completion, including after
/// cancellation. A submission ID is queue acceptance, not success.
pub trait RotationTransport {
    fn submit(&mut self, request: RotationRequest) -> Result<SubmissionId, RotationError>;
    fn poll(&mut self, id: SubmissionId, wait: Duration) -> Result<RotationPoll, RotationError>;
    fn cancel(&mut self, id: SubmissionId) -> Result<(), RotationError>;
    fn close(&mut self) -> Result<(), RotationError>;
}

pub struct UnsupportedRotationTransport;

impl RotationTransport for UnsupportedRotationTransport {
    fn submit(&mut self, _: RotationRequest) -> Result<SubmissionId, RotationError> {
        Err(DriverOutputError::UnsupportedPlatform.into())
    }
    fn poll(&mut self, _: SubmissionId, _: Duration) -> Result<RotationPoll, RotationError> {
        Err(DriverOutputError::UnsupportedPlatform.into())
    }
    fn cancel(&mut self, _: SubmissionId) -> Result<(), RotationError> {
        Err(DriverOutputError::UnsupportedPlatform.into())
    }
    fn close(&mut self) -> Result<(), RotationError> {
        Err(DriverOutputError::UnsupportedPlatform.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RotationDelivery {
    Pending(SubmissionId),
    Completed(RotationReply),
    Failed(RotationError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RotationReceipt {
    pub request: RotationRequest,
    pub delivery: RotationDelivery,
    pub cancel_requested: bool,
    /// Observation/cancel errors do not retire a pending kernel request.
    pub errors: Vec<RotationError>,
}

impl RotationReceipt {
    fn submit(transport: &mut impl RotationTransport, request: RotationRequest) -> Self {
        let delivery = match transport.submit(request) {
            Ok(id) => RotationDelivery::Pending(id),
            Err(error) => RotationDelivery::Failed(error),
        };
        Self {
            request,
            delivery,
            cancel_requested: false,
            errors: Vec::new(),
        }
    }

    pub fn pending(&self) -> bool {
        matches!(self.delivery, RotationDelivery::Pending(_))
    }

    pub fn driver_completed(&self) -> bool {
        matches!(self.delivery, RotationDelivery::Completed(_))
    }

    fn poll(&mut self, transport: &mut impl RotationTransport, wait: Duration) {
        let RotationDelivery::Pending(id) = self.delivery else {
            return;
        };
        match transport.poll(id, wait) {
            Ok(RotationPoll::Pending) => {}
            Err(error) => self.errors.push(error),
            Ok(RotationPoll::Completed(Err(error))) => {
                self.delivery = RotationDelivery::Failed(error);
            }
            Ok(RotationPoll::Completed(Ok(reply))) => {
                let valid = matches!(
                    (self.request, reply),
                    (RotationRequest::SetAngle(_), RotationReply::SetComplete)
                        | (RotationRequest::GetAngle, RotationReply::Angle { .. })
                );
                self.delivery = if valid {
                    RotationDelivery::Completed(reply)
                } else {
                    RotationDelivery::Failed(RotationError::UnexpectedReply)
                };
            }
        }
    }

    fn cancel(&mut self, transport: &mut impl RotationTransport) {
        let RotationDelivery::Pending(id) = self.delivery else {
            return;
        };
        if self.cancel_requested {
            return;
        }
        self.cancel_requested = true;
        if let Err(error) = transport.cancel(id) {
            self.errors.push(error);
        }
    }
}

/// One source request and its actual driver completion. A successful SET
/// does not submit an additional GET or manufacture an observed angle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RotationExecution {
    pub primary: RotationReceipt,
    pub stopped: Option<RotationError>,
}
impl RotationExecution {
    pub fn start(
        transport: &mut impl RotationTransport,
        request: RotationRequest,
        validate: &impl Fn() -> Result<(), RotationError>,
    ) -> Result<Self, RotationError> {
        validate()?;
        let primary = RotationReceipt::submit(transport, request);
        let mut result = Self {
            primary,
            stopped: None,
        };
        if let Err(error) = validate() {
            result.stop_and_cancel(transport, error);
        }
        Ok(result)
    }
    pub fn poll(
        &mut self,
        transport: &mut impl RotationTransport,
        wait: Duration,
        validate: &impl Fn() -> Result<(), RotationError>,
    ) {
        if self.stopped.is_none() {
            if let Err(error) = validate() {
                self.stop_and_cancel(transport, error);
            }
        }
        // A retired identity still owns pending kernel memory until drained.
        self.primary.poll(transport, wait);
        if self.stopped.is_none() {
            if let Err(error) = validate() {
                self.stop_and_cancel(transport, error);
            }
        }
    }
    fn stop_and_cancel(&mut self, transport: &mut impl RotationTransport, error: RotationError) {
        self.stopped.get_or_insert(error);
        self.primary.cancel(transport);
    }
    pub fn cancel(&mut self, transport: &mut impl RotationTransport) {
        self.stop_and_cancel(transport, RotationError::Canceled);
    }
    pub fn pending(&self) -> bool {
        self.primary.pending()
    }
    pub fn finished(&self) -> bool {
        !self.pending()
    }
    /// Closing never resets the configured angle or implies persistence.
    pub fn close_if_drained(
        &self,
        transport: &mut impl RotationTransport,
    ) -> Result<(), RotationError> {
        if !self.finished() {
            return Err(DriverOutputError::ExecutionPending.into());
        }
        transport.close()
    }
}

pub fn win32_error(operation: DriverOperation, code: u32) -> RotationError {
    DriverOutputError::Win32 { operation, code }.into()
}

#[cfg(test)]
#[path = "mapping_native_rotation_tests.rs"]
mod tests;

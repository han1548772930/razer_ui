//! Native mapping driver output, recovered from the current mapping DLL.
//!
//! The payload is a 32-byte Windows driver IOCTL input, never an HID report.
//! Curves and contributions stay in `mapping_native_analog`; OS handles and
//! overlapped I/O stay in the service adapter. See the native output audit.

use crate::mapping_native_analog::{
    AnalogMappingState, AnalogOutputState, GamepadAxis, JoystickBank, NativeAnalogOutput,
};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    time::{Duration, Instant},
};

pub const DRIVER_SEND_INPUT_IOCTL: u32 = 0x8888_3020;
pub const DRIVER_INPUT_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoystickButton(u8);

impl JoystickButton {
    pub fn new(button: u8) -> Result<Self, DriverOutputError> {
        if (1..=24).contains(&button) {
            Ok(Self(button))
        } else {
            Err(DriverOutputError::InvalidJoystickButton(button))
        }
    }

    pub fn number(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoystickAxisGroup {
    Axes,
    RotationAxes,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JoystickOutputState {
    pub buttons: [bool; 24],
    pub axes: [i8; 3],
    pub rotation_axes: [i8; 3],
    pub pov: [i8; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamepadOutputState {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left_thumb: [i16; 2],
    pub right_thumb: [i16; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MappingOutputState {
    pub joystick: JoystickOutputState,
    pub gamepad: GamepadOutputState,
}

impl MappingOutputState {
    /// An analog event changes these fields while preserving digital buttons
    /// and POV state, which belong to other mapping handlers.
    pub fn update_analog(&mut self, state: AnalogOutputState) {
        self.joystick.axes = state.axes;
        self.joystick.rotation_axes = state.rotation_axes;
        self.gamepad.left_trigger = state.left_trigger;
        self.gamepad.right_trigger = state.right_trigger;
        self.gamepad.left_thumb = state.left_thumb;
        self.gamepad.right_thumb = state.right_thumb;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverInput {
    JoystickButton {
        button: JoystickButton,
        pressed: bool,
    },
    JoystickAxes {
        group: JoystickAxisGroup,
        axes: [i8; 3],
    },
    JoystickPov([i8; 3]),
    Gamepad(GamepadOutputState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverInputReport {
    bytes: [u8; DRIVER_INPUT_BYTES],
}

impl DriverInputReport {
    pub(crate) fn from_native_input(bytes: [u8; DRIVER_INPUT_BYTES]) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8; DRIVER_INPUT_BYTES] {
        &self.bytes
    }

    pub fn ioctl(self) -> u32 {
        DRIVER_SEND_INPUT_IOCTL
    }
}

/// `0` points up, `2` right, `4` down, `6` left, and `8` is neutral.
/// Only the signs of X/Y matter; the native driver ignores POV Z.
pub fn joystick_pov_code(pov: [i8; 3]) -> u8 {
    match (pov[0].signum(), pov[1].signum()) {
        (0, -1) => 0,
        (1, -1) => 1,
        (1, 0) => 2,
        (1, 1) => 3,
        (0, 1) => 4,
        (-1, 1) => 5,
        (-1, 0) => 6,
        (-1, -1) => 7,
        _ => 8,
    }
}

pub fn encode_driver_input(input: DriverInput) -> DriverInputReport {
    let mut bytes = [0; DRIVER_INPUT_BYTES];
    let opcode = match input {
        DriverInput::JoystickButton { button, pressed } => {
            bytes[8] = u8::from(!pressed);
            bytes[9] = button.number();
            7u32
        }
        DriverInput::JoystickAxes { group, axes } => {
            for (slot, value) in bytes[8..11].iter_mut().zip(axes) {
                *slot = (i16::from(value) + 128) as u8;
            }
            match group {
                JoystickAxisGroup::Axes => 5,
                JoystickAxisGroup::RotationAxes => 6,
            }
        }
        DriverInput::JoystickPov(pov) => {
            bytes[8] = joystick_pov_code(pov);
            9
        }
        DriverInput::Gamepad(state) => {
            // The native source copies 4 bytes from +16 to input +10, then
            // 8 bytes from +20 to input +14. +8/+9 remain zero.
            bytes[10..12].copy_from_slice(&state.buttons.to_le_bytes());
            bytes[12] = state.left_trigger;
            bytes[13] = state.right_trigger;
            for (slot, value) in bytes[14..22]
                .chunks_exact_mut(2)
                .zip(state.left_thumb.into_iter().chain(state.right_thumb))
            {
                slot.copy_from_slice(&value.to_le_bytes());
            }
            12
        }
    };
    bytes[4..8].copy_from_slice(&opcode.to_le_bytes());
    DriverInputReport { bytes }
}

/// Preserve the recovered order: buttons 1..24, XYZ, rotation XYZ, POV,
/// then one complete gamepad payload. Unchanged groups produce no write.
pub fn changed_driver_inputs(
    previous: MappingOutputState,
    current: MappingOutputState,
) -> Vec<DriverInput> {
    let mut inputs = Vec::new();
    for (index, (&before, &after)) in previous
        .joystick
        .buttons
        .iter()
        .zip(&current.joystick.buttons)
        .enumerate()
    {
        if before != after {
            inputs.push(DriverInput::JoystickButton {
                button: JoystickButton((index + 1) as u8),
                pressed: after,
            });
        }
    }
    if previous.joystick.axes != current.joystick.axes {
        inputs.push(DriverInput::JoystickAxes {
            group: JoystickAxisGroup::Axes,
            axes: current.joystick.axes,
        });
    }
    if previous.joystick.rotation_axes != current.joystick.rotation_axes {
        inputs.push(DriverInput::JoystickAxes {
            group: JoystickAxisGroup::RotationAxes,
            axes: current.joystick.rotation_axes,
        });
    }
    if previous.joystick.pov != current.joystick.pov {
        inputs.push(DriverInput::JoystickPov(current.joystick.pov));
    }
    if previous.gamepad != current.gamepad {
        inputs.push(DriverInput::Gamepad(current.gamepad));
    }
    inputs
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverOperation {
    Open,
    CompletionEvent,
    Submit,
    Complete,
    Cancel,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriverOutputError {
    UnsupportedPlatform,
    InvalidPath,
    InvalidJoystickButton(u8),
    Closed,
    Disconnecting,
    NoExecution,
    ExecutionPending,
    UnknownSubmission(SubmissionId),
    SubmissionIdExhausted,
    Win32 {
        operation: DriverOperation,
        code: u32,
    },
}

impl fmt::Display for DriverOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                f.write_str("mapping output driver is unavailable on this platform")
            }
            Self::InvalidPath => {
                f.write_str("mapping driver requires an explicit nonempty ASCII path without NUL")
            }
            Self::InvalidJoystickButton(button) => {
                write!(f, "joystick button {button} is outside 1..24")
            }
            Self::Closed => f.write_str("mapping output driver is closed"),
            Self::Disconnecting => f.write_str("mapping output driver is disconnecting"),
            Self::NoExecution => f.write_str("no mapping output execution exists"),
            Self::ExecutionPending => f.write_str("mapping driver completion is still pending"),
            Self::UnknownSubmission(id) => write!(f, "unknown mapping driver submission {}", id.0),
            Self::SubmissionIdExhausted => f.write_str("mapping driver submission ID exhausted"),
            Self::Win32 { operation, code } => write!(
                f,
                "mapping driver {operation:?} failed with Win32 error {code}"
            ),
        }
    }
}

impl Error for DriverOutputError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubmissionId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionPoll {
    Pending,
    Completed(Result<(), DriverOutputError>),
}

/// `submit` returning an ID proves queue acceptance only. Even a synchronous
/// DeviceIoControl return must obtain final completion before reporting success.
/// Cancellation is a request; the same ID must still be drained to completion.
pub trait MappingOutputTransport {
    fn submit(&mut self, report: DriverInputReport) -> Result<SubmissionId, DriverOutputError>;
    fn poll_completion(
        &mut self,
        id: SubmissionId,
        wait: Duration,
    ) -> Result<CompletionPoll, DriverOutputError>;
    fn cancel(&mut self, id: SubmissionId) -> Result<(), DriverOutputError>;
    /// Implementations reject close while retained operations are outstanding.
    fn close(&mut self) -> Result<(), DriverOutputError>;
}

pub struct UnsupportedMappingOutputTransport;

impl MappingOutputTransport for UnsupportedMappingOutputTransport {
    fn submit(&mut self, _: DriverInputReport) -> Result<SubmissionId, DriverOutputError> {
        Err(DriverOutputError::UnsupportedPlatform)
    }
    fn poll_completion(
        &mut self,
        _: SubmissionId,
        _: Duration,
    ) -> Result<CompletionPoll, DriverOutputError> {
        Err(DriverOutputError::UnsupportedPlatform)
    }
    fn cancel(&mut self, _: SubmissionId) -> Result<(), DriverOutputError> {
        Err(DriverOutputError::UnsupportedPlatform)
    }
    fn close(&mut self) -> Result<(), DriverOutputError> {
        Err(DriverOutputError::UnsupportedPlatform)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReportDelivery {
    Pending(SubmissionId),
    Completed,
    Failed(DriverOutputError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportExecution {
    pub input: DriverInput,
    pub report: DriverInputReport,
    pub delivery: ReportDelivery,
    /// Poll/cancel errors do not retire an outstanding kernel operation.
    pub observation_errors: Vec<DriverOutputError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputExecution {
    pub previous_local_state: MappingOutputState,
    pub desired_local_state: MappingOutputState,
    pub reports: Vec<ReportExecution>,
}

impl OutputExecution {
    pub fn is_pending(&self) -> bool {
        self.reports
            .iter()
            .any(|r| matches!(r.delivery, ReportDelivery::Pending(_)))
    }

    /// Empty/unchanged output is a local no-op, not a driver acknowledgement.
    pub fn driver_confirmed(&self) -> bool {
        !self.reports.is_empty()
            && self
                .reports
                .iter()
                .all(|r| r.delivery == ReportDelivery::Completed)
    }

    fn submit(
        transport: &mut impl MappingOutputTransport,
        previous: MappingOutputState,
        desired: MappingOutputState,
    ) -> Self {
        let reports = changed_driver_inputs(previous, desired)
            .into_iter()
            .map(|input| {
                let report = encode_driver_input(input);
                let delivery = match transport.submit(report) {
                    Ok(id) => ReportDelivery::Pending(id),
                    Err(error) => ReportDelivery::Failed(error),
                };
                ReportExecution {
                    input,
                    report,
                    delivery,
                    observation_errors: Vec::new(),
                }
            })
            .collect();
        Self {
            previous_local_state: previous,
            desired_local_state: desired,
            reports,
        }
    }

    fn poll(&mut self, transport: &mut impl MappingOutputTransport, wait: Duration) {
        let started = Instant::now();
        for report in &mut self.reports {
            if let ReportDelivery::Pending(id) = report.delivery {
                match transport.poll_completion(id, wait.saturating_sub(started.elapsed())) {
                    Ok(CompletionPoll::Pending) => {}
                    Ok(CompletionPoll::Completed(Ok(()))) => {
                        report.delivery = ReportDelivery::Completed
                    }
                    Ok(CompletionPoll::Completed(Err(error))) => {
                        report.delivery = ReportDelivery::Failed(error)
                    }
                    Err(error) => report.observation_errors.push(error),
                }
            }
        }
    }

    fn cancel(&mut self, transport: &mut impl MappingOutputTransport) {
        for report in &mut self.reports {
            if let ReportDelivery::Pending(id) = report.delivery {
                if let Err(error) = transport.cancel(id) {
                    report.observation_errors.push(error);
                }
            }
        }
    }
}

pub struct AnalogEvent<'a> {
    pub input_id: &'a str,
    pub output: &'a NativeAnalogOutput,
    pub raw_actuation: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisconnectStage {
    DrainingCurrentOutput,
    ReleasingOutput,
    ClosingDriver,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisconnectExecution {
    pub stage: DisconnectStage,
    pub prior_output: Option<OutputExecution>,
    pub release_output: Option<OutputExecution>,
    pub close_error: Option<DriverOutputError>,
}

/// One ordered output session. Local snapshots advance as in the native event
/// batch; a failed submission is retained in its receipt, never retried silently
/// or mislabeled as confirmed device state. New batches await prior completion.
pub struct MappingOutputSession<T> {
    transport: T,
    analog: AnalogMappingState,
    active_analog: BTreeMap<(String, u8), NativeAnalogOutput>,
    local_output: MappingOutputState,
    last_execution: Option<OutputExecution>,
    disconnect: Option<DisconnectExecution>,
}

impl<T: MappingOutputTransport> MappingOutputSession<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            analog: AnalogMappingState::default(),
            active_analog: BTreeMap::new(),
            local_output: MappingOutputState::default(),
            last_execution: None,
            disconnect: None,
        }
    }

    pub fn local_output(&self) -> MappingOutputState {
        self.local_output
    }

    pub fn last_execution(&self) -> Option<&OutputExecution> {
        self.last_execution.as_ref()
    }

    fn ready(&self) -> Result<(), DriverOutputError> {
        if let Some(disconnect) = &self.disconnect {
            return Err(if disconnect.stage == DisconnectStage::Closed {
                DriverOutputError::Closed
            } else {
                DriverOutputError::Disconnecting
            });
        }
        if self
            .last_execution
            .as_ref()
            .is_some_and(OutputExecution::is_pending)
        {
            return Err(DriverOutputError::ExecutionPending);
        }
        Ok(())
    }

    fn flush(&mut self, previous: MappingOutputState, wait: Duration) -> OutputExecution {
        let mut execution =
            OutputExecution::submit(&mut self.transport, previous, self.local_output);
        execution.poll(&mut self.transport, wait);
        self.last_execution = Some(execution.clone());
        execution
    }

    /// Snapshot before the report batch, apply every event, then flush once.
    /// Multiple keys affecting one axis emit the final aggregate, not each key.
    pub fn analog_batch(
        &mut self,
        events: &[AnalogEvent<'_>],
        wait: Duration,
    ) -> Result<OutputExecution, DriverOutputError> {
        self.ready()?;
        let previous = self.local_output;
        for event in events {
            let destination = match event.output {
                NativeAnalogOutput::Joystick(curve) => match curve.bank {
                    JoystickBank::Axes => 0,
                    JoystickBank::RotationAxes => 1,
                },
                NativeAnalogOutput::Gamepad(curve) => match curve.axis {
                    GamepadAxis::LeftTrigger => 2,
                    GamepadAxis::RightTrigger => 3,
                    GamepadAxis::LeftThumbX => 4,
                    GamepadAxis::LeftThumbY => 5,
                    GamepadAxis::RightThumbX => 6,
                    GamepadAxis::RightThumbY => 7,
                },
            };
            let key = (event.input_id.to_owned(), destination);
            if event.raw_actuation == 0 {
                self.active_analog.remove(&key);
            } else {
                self.active_analog.insert(key, event.output.clone());
            }
            let output = self
                .analog
                .apply(event.input_id, event.output, event.raw_actuation);
            self.local_output.update_analog(output);
        }
        Ok(self.flush(previous, wait))
    }

    pub fn remove_analog_input(
        &mut self,
        input_id: &str,
        wait: Duration,
    ) -> Result<OutputExecution, DriverOutputError> {
        self.ready()?;
        let previous = self.local_output;
        self.active_analog.retain(|(key, _), _| key != input_id);
        self.local_output
            .update_analog(self.analog.remove_input(input_id));
        Ok(self.flush(previous, wait))
    }

    /// Submit complete typed state produced by the other native handlers.
    /// The caller owns its digital contribution counters; this method does not
    /// invent button events or read device state. The analog convenience path
    /// is for analog-only axis contributions; callers mixing digital and analog
    /// axis mappings must aggregate both before using this full-state API.
    pub fn output_state(
        &mut self,
        output: MappingOutputState,
        wait: Duration,
    ) -> Result<OutputExecution, DriverOutputError> {
        self.ready()?;
        let previous = self.local_output;
        self.local_output = output;
        Ok(self.flush(previous, wait))
    }

    /// Release tracked inputs and flush neutral state through the same driver
    /// chain. This is not the Xbox vtable Reset, which leaves its maps intact.
    pub fn clear(&mut self, wait: Duration) -> Result<OutputExecution, DriverOutputError> {
        self.ready()?;
        let previous = self.local_output;
        self.analog.clear();
        self.active_analog.clear();
        self.local_output = MappingOutputState::default();
        Ok(self.flush(previous, wait))
    }

    pub fn poll(&mut self, wait: Duration) -> Result<OutputExecution, DriverOutputError> {
        let execution = if let Some(disconnect) = &mut self.disconnect {
            match disconnect.stage {
                DisconnectStage::DrainingCurrentOutput => disconnect.prior_output.as_mut(),
                DisconnectStage::ReleasingOutput | DisconnectStage::ClosingDriver => {
                    disconnect.release_output.as_mut()
                }
                DisconnectStage::Closed => return Err(DriverOutputError::Closed),
            }
        } else {
            self.last_execution.as_mut()
        }
        .ok_or(DriverOutputError::NoExecution)?;
        execution.poll(&mut self.transport, wait);
        Ok(execution.clone())
    }

    pub fn cancel_pending(&mut self, wait: Duration) -> Result<OutputExecution, DriverOutputError> {
        let execution = if let Some(disconnect) = &mut self.disconnect {
            match disconnect.stage {
                DisconnectStage::DrainingCurrentOutput => disconnect.prior_output.as_mut(),
                DisconnectStage::ReleasingOutput | DisconnectStage::ClosingDriver => {
                    disconnect.release_output.as_mut()
                }
                DisconnectStage::Closed => return Err(DriverOutputError::Closed),
            }
        } else {
            self.last_execution.as_mut()
        }
        .ok_or(DriverOutputError::NoExecution)?;
        execution.cancel(&mut self.transport);
        execution.poll(&mut self.transport, wait);
        Ok(execution.clone())
    }

    /// Drain earlier writes, replay raw zero for active analog inputs, flush,
    /// drain its completion, then close. Native zero-point curves can keep an
    /// output nonzero on release; do not substitute a hard reset for that case.
    /// A timeout keeps the session and kernel buffers outstanding.
    /// Release errors remain in the returned receipt even when close succeeds.
    pub fn disconnect(&mut self, wait: Duration) -> DisconnectExecution {
        let started = Instant::now();
        let disconnect = self.disconnect.get_or_insert_with(|| DisconnectExecution {
            stage: DisconnectStage::DrainingCurrentOutput,
            prior_output: self.last_execution.take(),
            release_output: None,
            close_error: None,
        });
        if disconnect.stage == DisconnectStage::DrainingCurrentOutput {
            if let Some(prior) = &mut disconnect.prior_output {
                prior.poll(&mut self.transport, wait);
                if prior.is_pending() {
                    return disconnect.clone();
                }
            }
            let previous = self.local_output;
            for ((input_id, _), curve) in &self.active_analog {
                self.analog.apply(input_id, curve, 0);
            }
            self.active_analog.clear();
            self.local_output = MappingOutputState::default();
            self.local_output.update_analog(self.analog.output());
            disconnect.release_output = Some(OutputExecution::submit(
                &mut self.transport,
                previous,
                self.local_output,
            ));
            disconnect.stage = DisconnectStage::ReleasingOutput;
        }
        if disconnect.stage == DisconnectStage::ReleasingOutput {
            let release = disconnect
                .release_output
                .as_mut()
                .expect("release stage holds an execution");
            release.poll(&mut self.transport, wait.saturating_sub(started.elapsed()));
            if release.is_pending() {
                return disconnect.clone();
            }
            disconnect.stage = DisconnectStage::ClosingDriver;
        }
        if disconnect.stage == DisconnectStage::ClosingDriver {
            match self.transport.close() {
                Ok(()) => {
                    disconnect.stage = DisconnectStage::Closed;
                    disconnect.close_error = None;
                }
                Err(error) => disconnect.close_error = Some(error),
            }
        }
        disconnect.clone()
    }
}

#[cfg(test)]
#[path = "mapping_native_output_tests.rs"]
mod tests;

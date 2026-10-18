//! Typed events from the mapping driver's persistent `0x88883018` read.
//!
//! IDA/Hex-Rays RVA `0x114850` dispatches one union event per completion,
//! after rearming the other 304-byte buffer. It does not check the returned
//! byte count. This decoder checks bounds and retains opaque bytes instead of
//! inventing a batch count or requiring zero padding. Analog actuation comes
//! from a separate HID report callback, never from this stream.
//!
//! Hook resolution is an explicit caller boundary: product/global lookup,
//! timers and unsupported mapping types are not inferred here. Notify and
//! redirect callbacks are observations, not requests to remap and echo input.
//! Desired mapping state, queue acceptance and operation completion remain
//! distinct; no returned plan or accepted operation claims device delivery.

use crate::{
    mapping_native_analog::NativeAnalogOutput,
    mapping_native_digital::{
        NativeDigitalError, NativeDigitalMappingState, NativeDigitalOutput, NativeMappingDelta,
    },
    mapping_native_input::{
        KeyboardInput, MouseInput, NativeInput, encode_keyboard_sequence, encode_native_input,
    },
    mapping_native_output::{
        DriverInput, DriverInputReport, changed_driver_inputs, encode_driver_input,
    },
    mapping_native_session::{
        DRIVER_READ_INPUT_OUTPUT_BYTES, NativeCapability, NativeCapabilitySet, NativeCompletion,
        NativeInputPoll, NativeSession, NativeSessionError, NativeSessionState,
        NativeSessionTransport, NativeSubmission, NativeSubmissionId, NativeTransportError,
    },
};
use std::{collections::BTreeMap, fmt, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeStreamKind {
    Hook,
    Notify,
    Redirect,
    Unknown(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeKeyboardEvent {
    pub unit_id: u16,
    pub scancode: u16,
    pub flags: u16,
    pub reserved: u16,
    pub extra_information: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeMouseEvent {
    pub unit_id: u16,
    pub flags: u16,
    pub button_flags: u16,
    pub button_data: i16,
    pub raw_buttons: u32,
    pub movement: [i32; 2],
    pub extra_information: u32,
}

impl NativeMouseEvent {
    pub const fn packed_buttons(self) -> u32 {
        self.button_flags as u32 | ((self.button_data as u16 as u32) << 16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeStreamInput {
    Keyboard(NativeKeyboardEvent),
    Mouse(NativeMouseEvent),
    /// Only kind 1/type 4 uses these two bytes. Their boolean meaning has not
    /// been inferred: the native callback receives the original byte values.
    RedirectState {
        first: u8,
        second: u8,
    },
    Unknown {
        input_type: Option<u32>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStreamPacket {
    pub kind: NativeStreamKind,
    pub input: NativeStreamInput,
    raw: Vec<u8>,
}

impl NativeStreamPacket {
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeStreamDecodeError {
    Oversized {
        actual: usize,
        capacity: usize,
    },
    Truncated {
        branch: &'static str,
        actual: usize,
        required: usize,
    },
}

impl fmt::Display for NativeStreamDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { actual, capacity } => {
                write!(
                    f,
                    "mapping input has {actual} bytes, capacity is {capacity}"
                )
            }
            Self::Truncated {
                branch,
                actual,
                required,
            } => {
                write!(
                    f,
                    "mapping {branch} needs {required} bytes, received {actual}"
                )
            }
        }
    }
}

impl std::error::Error for NativeStreamDecodeError {}

fn require(
    bytes: &[u8],
    required: usize,
    branch: &'static str,
) -> Result<(), NativeStreamDecodeError> {
    if bytes.len() < required {
        Err(NativeStreamDecodeError::Truncated {
            branch,
            actual: bytes.len(),
            required,
        })
    } else {
        Ok(())
    }
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn dword(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

/// Offsets are relative to the output buffer, not its preceding OVERLAPPED.
/// Keyboard requires the whole 12-byte KEYBOARD_INPUT_DATA union (through
/// offset 31); the native constructor itself consumes only through offset 25.
/// This conservative Rust bound is distinct from the unchecked native code.
pub fn decode_native_stream_packet(
    bytes: &[u8],
) -> Result<NativeStreamPacket, NativeStreamDecodeError> {
    if bytes.len() > DRIVER_READ_INPUT_OUTPUT_BYTES {
        return Err(NativeStreamDecodeError::Oversized {
            actual: bytes.len(),
            capacity: DRIVER_READ_INPUT_OUTPUT_BYTES,
        });
    }
    require(bytes, 12, "event kind")?;
    let kind = match dword(bytes, 8) {
        1 => NativeStreamKind::Redirect,
        2 => NativeStreamKind::Hook,
        4 => NativeStreamKind::Notify,
        value => NativeStreamKind::Unknown(value),
    };
    let input = if matches!(kind, NativeStreamKind::Unknown(_)) {
        NativeStreamInput::Unknown {
            input_type: (bytes.len() >= 20).then(|| dword(bytes, 16)),
        }
    } else {
        require(bytes, 20, "input type")?;
        match dword(bytes, 16) {
            1 => {
                require(bytes, 32, "keyboard input")?;
                NativeStreamInput::Keyboard(NativeKeyboardEvent {
                    unit_id: word(bytes, 20),
                    scancode: word(bytes, 22),
                    flags: word(bytes, 24),
                    reserved: word(bytes, 26),
                    extra_information: dword(bytes, 28),
                })
            }
            2 => {
                require(bytes, 44, "mouse input")?;
                NativeStreamInput::Mouse(NativeMouseEvent {
                    unit_id: word(bytes, 20),
                    flags: word(bytes, 22),
                    button_flags: word(bytes, 24),
                    button_data: word(bytes, 26) as i16,
                    raw_buttons: dword(bytes, 28),
                    movement: [dword(bytes, 32) as i32, dword(bytes, 36) as i32],
                    extra_information: dword(bytes, 40),
                })
            }
            4 if kind == NativeStreamKind::Redirect => {
                require(bytes, 43, "redirect state")?;
                NativeStreamInput::RedirectState {
                    first: bytes[41],
                    second: bytes[42],
                }
            }
            value => NativeStreamInput::Unknown {
                input_type: Some(value),
            },
        }
    };
    Ok(NativeStreamPacket {
        kind,
        input,
        raw: bytes.to_vec(),
    })
}

/// RVA `0x11C460` samples eight GetKeyState values. The caller supplies the
/// actual snapshot; pure code never substitutes synthetic OS observations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeModifierSnapshot {
    pub left_alt: bool,
    pub left_control: bool,
    pub left_shift: bool,
    pub left_windows: bool,
    pub right_alt: bool,
    pub right_control: bool,
    pub right_shift: bool,
    pub right_windows: bool,
}

impl NativeModifierSnapshot {
    pub const fn source_bits(self) -> u32 {
        (self.left_alt as u32) * 0x100
            | (self.left_control as u32) * 0x200
            | (self.left_shift as u32) * 0x400
            | (self.left_windows as u32) * 0x800
            | (self.right_alt as u32) * 0x10
            | (self.right_control as u32) * 0x20
            | (self.right_shift as u32) * 0x40
            | (self.right_windows as u32) * 0x80
    }
}

impl NativeStreamInput {
    /// RVAs `0x114E3D`/`0x114EBF` copy only the mapping key fields. Physical
    /// motion and extra info follow a separate coalescing path. Mouse data is
    /// clamped as signed WORD to -120..=120 before key lookup.
    pub fn normalized(
        self,
        preserve_unit: bool,
        modifiers: NativeModifierSnapshot,
    ) -> Option<NativeInput> {
        let modifiers = modifiers.source_bits();
        match self {
            Self::Keyboard(raw) => Some(NativeInput::Keyboard(KeyboardInput {
                unit_id: if preserve_unit { raw.unit_id } else { 0 },
                scancode: raw.scancode,
                flag: raw.flags,
                modifiers,
                ..KeyboardInput::default()
            })),
            Self::Mouse(raw) => Some(NativeInput::Mouse(MouseInput {
                unit_id: if preserve_unit { raw.unit_id } else { 0 },
                id: raw.button_flags,
                data: raw.button_data.clamp(-120, 120) as u16,
                modifiers,
                ..MouseInput::default()
            })),
            Self::RedirectState { .. } | Self::Unknown { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeEventOutput {
    Input(NativeInput),
    /// Pairing/flags are the resolver's source-verified responsibility. Do
    /// not turn every notify or repeated key-down into a new digital press.
    Digital {
        input_id: String,
        output: NativeDigitalOutput,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHookResolution {
    PassThrough,
    Mapped(Vec<NativeEventOutput>),
    Suppress,
    /// Missing product/global rules or an unsupported mapping branch are
    /// explicit; they do not silently claim a successful mapped output.
    Unresolved(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeObservedEvent {
    Notify(NativeInput),
    Redirect(NativeInput),
    RedirectState { first: u8, second: u8 },
}

pub trait NativeStreamHandler {
    fn resolve_hook(&mut self, input: NativeInput) -> NativeHookResolution;
    fn observe(&mut self, event: NativeObservedEvent);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeEventDisposition {
    PassedThrough,
    Mapped,
    Suppressed,
    Unresolved(String),
    Notified,
    Redirected,
    RedirectState,
    UnknownEvent,
    UnknownInput,
    /// The source tests the full packed DWORD, including button data.
    EmptyMouseRedirect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeEventIssue {
    AbsoluteMouseMovement,
    MissingCapabilities(NativeCapabilitySet),
    Digital(NativeDigitalError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEventReportKind {
    Keyboard,
    Mouse,
    Display,
    Joystick,
    Gamepad,
    MouseMovement,
}

impl NativeEventReportKind {
    fn capability(self) -> NativeCapability {
        match self {
            Self::Keyboard => NativeCapability::KeyboardInput,
            Self::Mouse | Self::MouseMovement => NativeCapability::MouseInput,
            Self::Display => NativeCapability::DisplayInput,
            Self::Joystick => NativeCapability::JoystickOutput,
            Self::Gamepad => NativeCapability::GamepadOutput,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeEventSubmitError {
    Session(NativeSessionError),
    MissingCapabilities(NativeCapabilitySet),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeEventReportReceipt {
    pub kind: NativeEventReportKind,
    pub report: DriverInputReport,
    /// Ok is queue acceptance only; poll_outputs observes final completion.
    pub submission: Result<NativeSubmission, NativeEventSubmitError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeEventDispatch {
    pub packet: NativeStreamPacket,
    pub disposition: NativeEventDisposition,
    pub reports: Vec<NativeEventReportReceipt>,
    pub issues: Vec<NativeEventIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeEventPoll {
    Pending,
    ReadFailed(NativeTransportError),
    Completed {
        rearmed: Result<NativeSubmissionId, NativeTransportError>,
        dispatch: Result<NativeEventDispatch, NativeStreamDecodeError>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeEventOutputObservation {
    pub receipt: NativeEventReportReceipt,
    /// An observation error/Pending retains ownership of accepted buffers.
    pub completion: Result<NativeCompletion, NativeSessionError>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeEventOutputPoll {
    pub observations: Vec<NativeEventOutputObservation>,
    pub followup_reports: Vec<NativeEventReportReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAnalogEventDispatch {
    pub delta: NativeMappingDelta,
    pub reports: Vec<NativeEventReportReceipt>,
}

/// One source-resolved analog contribution within a complete hardware frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAnalogBatchUpdate {
    pub input_id: String,
    pub output: NativeAnalogOutput,
    pub raw_actuation: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeAnalogBatchError {
    Stopping,
    SessionState(NativeSessionState),
    MissingCapabilities(NativeCapabilitySet),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct QueuedMouseMovement {
    movement: [i32; 2],
    flags: u16,
    extra_information: u32,
}

/// Per-device state and accepted report ownership. This can use the Windows
/// session adapter or a simulated transport. Handler callbacks run after the
/// session has rearmed; real thread scheduling belongs to the caller.
#[derive(Default)]
pub struct NativeEventRuntime {
    desired: NativeDigitalMappingState,
    pending_outputs: BTreeMap<NativeSubmissionId, NativeEventReportReceipt>,
    queued_movement: Option<QueuedMouseMovement>,
    movement_inflight: Option<NativeSubmissionId>,
    stopping: bool,
}

impl NativeEventRuntime {
    pub fn desired_state(&self) -> &NativeDigitalMappingState {
        &self.desired
    }

    pub fn pending_output_count(&self) -> usize {
        self.pending_outputs.len()
    }

    pub fn poll<T: NativeSessionTransport, H: NativeStreamHandler>(
        &mut self,
        session: &mut NativeSession<T>,
        handler: &mut H,
        wait: Duration,
        preserve_unit: bool,
        modifiers: NativeModifierSnapshot,
    ) -> Result<NativeEventPoll, NativeSessionError> {
        match session.poll_input(wait)? {
            NativeInputPoll::Pending => Ok(NativeEventPoll::Pending),
            NativeInputPoll::Failed(error) => Ok(NativeEventPoll::ReadFailed(error)),
            NativeInputPoll::Completed { output, rearmed } => {
                // A completed event remains observable even if the new read
                // failed to queue; subsequent writes then carry that failure.
                let dispatch = decode_native_stream_packet(&output).map(|packet| {
                    self.dispatch(session, handler, packet, preserve_unit, modifiers)
                });
                Ok(NativeEventPoll::Completed { rearmed, dispatch })
            }
        }
    }

    pub fn dispatch<T: NativeSessionTransport, H: NativeStreamHandler>(
        &mut self,
        session: &mut NativeSession<T>,
        handler: &mut H,
        packet: NativeStreamPacket,
        preserve_unit: bool,
        modifiers: NativeModifierSnapshot,
    ) -> NativeEventDispatch {
        let mut result = NativeEventDispatch {
            packet,
            disposition: NativeEventDisposition::UnknownInput,
            reports: Vec::new(),
            issues: Vec::new(),
        };
        if matches!(result.packet.kind, NativeStreamKind::Unknown(_)) {
            result.disposition = NativeEventDisposition::UnknownEvent;
            return result;
        }
        if let NativeStreamInput::RedirectState { first, second } = result.packet.input {
            handler.observe(NativeObservedEvent::RedirectState { first, second });
            result.disposition = NativeEventDisposition::RedirectState;
            return result;
        }
        if let NativeStreamInput::Mouse(raw) = result.packet.input {
            if result.packet.kind == NativeStreamKind::Redirect && raw.packed_buttons() == 0 {
                result.disposition = NativeEventDisposition::EmptyMouseRedirect;
                return result;
            }
        }
        let (reports, issues) = self.physical_motion(session, &result.packet);
        result.reports.extend(reports);
        result.issues.extend(issues);
        let Some(input) = result.packet.input.normalized(preserve_unit, modifiers) else {
            return result;
        };
        match result.packet.kind {
            NativeStreamKind::Hook => match handler.resolve_hook(input) {
                NativeHookResolution::PassThrough => {
                    result.disposition = NativeEventDisposition::PassedThrough;
                    result
                        .reports
                        .extend(self.submit_native_input(session, input));
                }
                NativeHookResolution::Mapped(outputs) => {
                    result.disposition = NativeEventDisposition::Mapped;
                    for output in outputs {
                        match output {
                            NativeEventOutput::Input(input) => {
                                result
                                    .reports
                                    .extend(self.submit_native_input(session, input));
                            }
                            NativeEventOutput::Digital { input_id, output } => {
                                let kind = digital_kind(output);
                                if let Err(missing) =
                                    self.require_capabilities(session, kind.capability().into())
                                {
                                    result
                                        .issues
                                        .push(NativeEventIssue::MissingCapabilities(missing));
                                    continue;
                                }
                                match self.desired.apply_digital(&input_id, output) {
                                    Ok(delta) => {
                                        result.reports.extend(self.submit_delta(session, &delta))
                                    }
                                    Err(error) => {
                                        result.issues.push(NativeEventIssue::Digital(error))
                                    }
                                }
                            }
                        }
                    }
                }
                NativeHookResolution::Suppress => {
                    result.disposition = NativeEventDisposition::Suppressed
                }
                NativeHookResolution::Unresolved(reason) => {
                    result.disposition = NativeEventDisposition::Unresolved(reason)
                }
            },
            NativeStreamKind::Notify => {
                handler.observe(NativeObservedEvent::Notify(input));
                result.disposition = NativeEventDisposition::Notified;
            }
            NativeStreamKind::Redirect => {
                handler.observe(NativeObservedEvent::Redirect(input));
                result.disposition = NativeEventDisposition::Redirected;
            }
            NativeStreamKind::Unknown(_) => unreachable!(),
        }
        result
    }

    /// The retained driver owner calls this before handing button lookup to
    /// a device worker. Motion and mapped button output share one session.
    pub fn physical_motion<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
        packet: &NativeStreamPacket,
    ) -> (Vec<NativeEventReportReceipt>, Vec<NativeEventIssue>) {
        let mut reports = Vec::new();
        let mut issues = Vec::new();
        if let NativeStreamInput::Mouse(raw) = packet.input
            && packet.kind == NativeStreamKind::Hook
            && raw.button_flags != 0
            && raw.movement != [0, 0]
        {
            if raw.flags & 1 != 0 {
                issues.push(NativeEventIssue::AbsoluteMouseMovement);
            } else if !self.stopping {
                self.queue_movement(raw);
                if self.movement_inflight.is_none() {
                    reports.extend(self.flush_movement(session));
                }
            }
        }
        (reports, issues)
    }

    /// Input must already have been decoded/verified by the separate hardware
    /// HID callback. This does not imply an analog format for the 304-byte read.
    pub fn apply_analog<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
        input_id: &str,
        output: &NativeAnalogOutput,
        raw_actuation: u16,
    ) -> Result<NativeAnalogEventDispatch, NativeEventIssue> {
        let kind = match output {
            NativeAnalogOutput::Joystick(_) => NativeEventReportKind::Joystick,
            NativeAnalogOutput::Gamepad(_) => NativeEventReportKind::Gamepad,
        };
        self.require_capabilities(
            session,
            NativeCapabilitySet::from(NativeCapability::AnalogOutput).with(kind.capability()),
        )
        .map_err(NativeEventIssue::MissingCapabilities)?;
        let delta = self.desired.apply_analog(input_id, output, raw_actuation);
        let reports = self.submit_delta(session, &delta);
        Ok(NativeAnalogEventDispatch { delta, reports })
    }

    /// The HID decoders process all current keys and missing-key releases
    /// before publishing joystick/gamepad changes. Validate the whole batch
    /// before altering shared contribution state; emit only its final diff.
    /// Accepted operations retain the same output ownership/completion path
    /// as digital events and single analog updates.
    pub fn apply_analog_batch<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
        updates: &[NativeAnalogBatchUpdate],
    ) -> Result<NativeAnalogEventDispatch, NativeAnalogBatchError> {
        if self.stopping {
            return Err(NativeAnalogBatchError::Stopping);
        }
        if session.state() != NativeSessionState::Connected {
            return Err(NativeAnalogBatchError::SessionState(session.state()));
        }
        let mut required = NativeCapabilitySet::empty();
        for update in updates {
            required = required
                .with(NativeCapability::AnalogOutput)
                .with(match update.output {
                    NativeAnalogOutput::Joystick(_) => NativeCapability::JoystickOutput,
                    NativeAnalogOutput::Gamepad(_) => NativeCapability::GamepadOutput,
                });
        }
        self.require_capabilities(session, required)
            .map_err(NativeAnalogBatchError::MissingCapabilities)?;
        let previous = self.desired.output();
        for update in updates {
            self.desired
                .apply_analog(&update.input_id, &update.output, update.raw_actuation);
        }
        let desired = self.desired.output();
        let delta = NativeMappingDelta {
            previous,
            desired,
            requests: changed_driver_inputs(previous, desired),
        };
        let reports = self.submit_delta(session, &delta);
        Ok(NativeAnalogEventDispatch { delta, reports })
    }

    fn require_capabilities<T: NativeSessionTransport>(
        &self,
        session: &NativeSession<T>,
        required: NativeCapabilitySet,
    ) -> Result<(), NativeCapabilitySet> {
        let missing = session.route().capabilities().missing(required);
        if missing.bits() == 0 { Ok(()) } else { Err(missing) }
    }

    fn submit_native_input<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
        input: NativeInput,
    ) -> Vec<NativeEventReportReceipt> {
        let (kind, reports) = match input {
            NativeInput::Keyboard(input) => (
                NativeEventReportKind::Keyboard,
                encode_keyboard_sequence(input),
            ),
            NativeInput::Mouse(_) => (
                NativeEventReportKind::Mouse,
                vec![encode_native_input(input)],
            ),
            NativeInput::Display(_) => (
                NativeEventReportKind::Display,
                vec![encode_native_input(input)],
            ),
        };
        let mut receipts = Vec::new();
        for report in reports {
            let receipt = self.submit_report(session, kind, report);
            let accepted = receipt.submission.is_ok();
            receipts.push(receipt);
            if !accepted {
                break;
            } // No E1 continuation after primary rejection.
        }
        receipts
    }

    fn submit_delta<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
        delta: &NativeMappingDelta,
    ) -> Vec<NativeEventReportReceipt> {
        delta
            .requests
            .iter()
            .map(|&input| {
                let kind = match input {
                    DriverInput::Gamepad(_) => NativeEventReportKind::Gamepad,
                    _ => NativeEventReportKind::Joystick,
                };
                self.submit_report(session, kind, encode_driver_input(input))
            })
            .collect()
    }

    fn submit_report<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
        kind: NativeEventReportKind,
        report: DriverInputReport,
    ) -> NativeEventReportReceipt {
        let submission = match self.require_capabilities(session, kind.capability().into()) {
            Err(missing) => Err(NativeEventSubmitError::MissingCapabilities(missing)),
            Ok(()) => session
                .submit_input_report(report)
                .map_err(NativeEventSubmitError::Session),
        };
        let receipt = NativeEventReportReceipt {
            kind,
            report,
            submission,
        };
        if let Ok(submission) = &receipt.submission {
            self.pending_outputs
                .insert(submission.id(), receipt.clone());
        }
        receipt
    }

    fn queue_movement(&mut self, raw: NativeMouseEvent) {
        let queued = self.queued_movement.get_or_insert_with(Default::default);
        for axis in 0..2 {
            queued.movement[axis] = queued.movement[axis].wrapping_add(raw.movement[axis]);
        }
        queued.flags = raw.flags;
        queued.extra_information = raw.extra_information;
    }

    fn flush_movement<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
    ) -> Option<NativeEventReportReceipt> {
        let queued = self.queued_movement.take()?;
        // RVA 0x114624 clears the accumulator before attempting submission.
        // This physical extra-info DWORD is not the mapping modifier transform.
        let mut bytes = [0; 32];
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[10..12].copy_from_slice(&queued.flags.to_le_bytes());
        bytes[20..24].copy_from_slice(&queued.movement[0].to_le_bytes());
        bytes[24..28].copy_from_slice(&queued.movement[1].to_le_bytes());
        bytes[28..32].copy_from_slice(&queued.extra_information.to_le_bytes());
        let receipt = self.submit_report(
            session,
            NativeEventReportKind::MouseMovement,
            DriverInputReport::from_native_input(bytes),
        );
        self.movement_inflight = receipt.submission.as_ref().ok().map(NativeSubmission::id);
        Some(receipt)
    }

    /// Poll outputs independently of the persistent read. RVA 0x114840 clears
    /// movement in-flight and flushes queued movement on terminal completion,
    /// regardless of the success byte. Observation errors retain it. During
    /// disconnect there is no follow-up submission, even after cancellation.
    pub fn poll_outputs<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
        wait: Duration,
    ) -> NativeEventOutputPoll {
        let ids: Vec<_> = self.pending_outputs.keys().copied().collect();
        let mut result = NativeEventOutputPoll::default();
        for id in ids {
            let receipt = self.pending_outputs[&id].clone();
            let completion = session.poll_submission(id, wait);
            if matches!(completion, Ok(NativeCompletion::Completed(_))) {
                self.pending_outputs.remove(&id);
                if self.movement_inflight == Some(id) {
                    self.movement_inflight = None;
                    if !self.stopping && session.state() == NativeSessionState::Connected {
                        result.followup_reports.extend(self.flush_movement(session));
                    }
                }
            }
            result.observations.push(NativeEventOutputObservation {
                receipt,
                completion,
            });
        }
        result
    }

    /// Stop coalescing before requesting cancellation. Poll output receipts
    /// before session.poll_disconnect so each terminal result remains visible.
    /// The stream read is drained by the session; no canceled packet is decoded.
    pub fn begin_disconnect<T: NativeSessionTransport>(
        &mut self,
        session: &mut NativeSession<T>,
    ) -> Result<(), NativeSessionError> {
        session.begin_disconnect()?;
        self.stopping = true;
        self.queued_movement = None;
        Ok(())
    }
}

fn digital_kind(output: NativeDigitalOutput) -> NativeEventReportKind {
    match output {
        NativeDigitalOutput::GamepadButtons { .. } | NativeDigitalOutput::GamepadAxis { .. } => {
            NativeEventReportKind::Gamepad
        }
        NativeDigitalOutput::JoystickButton { .. } | NativeDigitalOutput::JoystickAxes { .. } => {
            NativeEventReportKind::Joystick
        }
    }
}

#[cfg(test)]
#[path = "mapping_native_events_tests.rs"]
mod tests;

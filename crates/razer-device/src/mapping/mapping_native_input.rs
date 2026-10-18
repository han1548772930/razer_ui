//! Native keyboard, mouse and display input packets from `mapping_engine.dll`.
//!
//! `DriverImpl::SendInput` accepts the same zeroed 32-byte IOCTL buffer used by
//! [`mapping_native_output`](crate::mapping_native_output), but its input
//! objects are distinct from joystick/Xbox state:
//!
//! * MappingDataKeyboardWin (type 21) writes opcode 1, a unit/scancode/flag
//!   tuple at byte 8, and the recovered modifier transform at byte 16.
//! * MappingDataMouseWin (type 27) writes opcode 2, its 16-byte native input
//!   object at byte 8, Y movement at byte 24, and transformed
//!   modifiers at byte 28. Mouse `data` is kept as a signed 16-bit bit pattern
//!   (`-120` is sent as `0xff88`).
//! * MappingDataDisplay (type 14) writes opcode 10 and accepts only source ids
//!   3, 4 and 5, which map to payload words 112, 111 and 0.
//!
//! The packet encoders are pure Rust. They do not open a driver or invoke the
//! vendor DLL. Submission/completion remains the transport concern of the
//! mapping transport. Failed primary requests can be passed to the recovered
//! operating-system fallback; its result stays separate from driver delivery.
//! The macro keyboard executor owns repeat timers; the platform adapter owns
//! driver discovery. These packets encode inputs selected by those owners.

use crate::mapping_native_output::{
    CompletionPoll, DriverOutputError, MappingOutputTransport, ReportDelivery,
};
pub use crate::mapping_native_output::{
    DRIVER_INPUT_BYTES, DRIVER_SEND_INPUT_IOCTL, DriverInputReport as NativeInputReport,
};
use std::{
    fmt,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyboardInput {
    pub unit_id: u16,
    pub scancode: u16,
    pub flag: u16,
    pub reserved: u16,
    pub modifiers: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MouseInput {
    pub unit_id: u16,
    pub flags: u16,
    pub id: u16,
    pub data: u16,
    pub raw_buttons: u32,
    pub movement: [i32; 2],
    pub modifiers: u32,
}

impl MouseInput {
    /// Preserve a signed wheel delta as the native `WORD` bit pattern.
    pub const fn signed_data(data: i16) -> u16 {
        data as u16
    }

    pub const fn signed_wheel_delta(self) -> i16 {
        self.data as i16
    }

    pub const fn packed_buttons(self) -> u32 {
        self.id as u32 | ((self.data as u32) << 16)
    }

    /// MappingDataMouseWin's virtual validation accepts individual button
    /// pairs and either wheel id. Combined button masks are not accepted by
    /// its JSON mapping constructor even though SendInput copies raw fields.
    pub const fn mapping_id_valid(self) -> bool {
        matches!(self.id, 1024 | 2048)
            || matches!(
                self.packed_buttons(),
                1 | 2
                    | 4
                    | 8
                    | 16
                    | 32
                    | 64
                    | 128
                    | 256
                    | 512
                    | 0x10000
                    | 0x20000
                    | 0x40000
                    | 0x80000
            )
    }

    pub const fn is_pressed(self) -> bool {
        matches!(
            self.packed_buttons(),
            1 | 4 | 16 | 64 | 256 | 0x10000 | 0x40000
        )
    }

    pub const fn is_released(self) -> bool {
        matches!(
            self.packed_buttons(),
            2 | 8 | 32 | 128 | 512 | 0x20000 | 0x80000
        )
    }

    pub fn pressed(mut self) -> Self {
        let value = match self.packed_buttons() {
            2 => 1,
            8 => 4,
            32 => 16,
            128 => 64,
            512 => 256,
            0x20000 => 0x10000,
            0x80000 => 0x40000,
            other => other,
        };
        self.id = value as u16;
        self.data = (value >> 16) as u16;
        self
    }

    pub fn released(mut self) -> Self {
        let value = match self.packed_buttons() {
            1 => 2,
            4 => 8,
            16 => 32,
            64 => 128,
            256 => 512,
            0x10000 => 0x20000,
            0x40000 => 0x80000,
            other => other,
        };
        self.id = value as u16;
        self.data = (value >> 16) as u16;
        self
    }
}

impl KeyboardInput {
    pub const fn is_released(self) -> bool {
        self.flag & 1 != 0
    }

    pub fn pressed(mut self) -> Self {
        self.flag &= !1;
        self
    }

    pub fn released(mut self) -> Self {
        self.flag |= 1;
        self
    }

    pub fn native_continuation(self) -> Option<Self> {
        (self.scancode == 29 && self.flag & 4 != 0).then(|| Self {
            unit_id: self.unit_id,
            scancode: 69,
            flag: self.flag & 1,
            reserved: 0,
            modifiers: native_modifier_bits(self.modifiers),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayInput {
    SourceId3,
    SourceId4,
    SourceId5,
}

impl DisplayInput {
    pub const fn source_id(self) -> u32 {
        match self {
            Self::SourceId3 => 3,
            Self::SourceId4 => 4,
            Self::SourceId5 => 5,
        }
    }

    pub const fn payload_word(self) -> u16 {
        match self {
            Self::SourceId3 => 112,
            Self::SourceId4 => 111,
            Self::SourceId5 => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInput {
    Keyboard(KeyboardInput),
    Mouse(MouseInput),
    Display(DisplayInput),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeInputError {
    UnsupportedDisplaySource(u32),
    UnsupportedDisplayPayload(u16),
    InvalidPacketLength(usize),
    UnsupportedOpcode(u32),
    NonzeroReserved { offset: usize },
    InvalidDriverModifiers(u32),
}

impl fmt::Display for NativeInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedDisplaySource(id) => {
                write!(
                    f,
                    "mapping display source id {id} is not accepted by native opcode 10"
                )
            }
            Self::UnsupportedDisplayPayload(value) => {
                write!(f, "unsupported native display word {value}")
            }
            Self::InvalidPacketLength(size) => {
                write!(f, "native input requires 32 bytes, received {size}")
            }
            Self::UnsupportedOpcode(opcode) => {
                write!(f, "unsupported native input opcode {opcode}")
            }
            Self::NonzeroReserved { offset } => {
                write!(f, "nonzero native input reserved byte at {offset}")
            }
            Self::InvalidDriverModifiers(bits) => write!(
                f,
                "native input driver modifier bits {bits:#x} contain unmapped bits"
            ),
        }
    }
}

impl std::error::Error for NativeInputError {}

/// Exact RVA `0x1219DC` modifier conversion recovered from Hex-Rays.
///
/// The six source bits are intentionally sparse. In particular, do not use a
/// generic Windows `KEYEVENTF_*` mask here: the DLL maps bits 4, 5, 6, 8, 9
/// and 10 to output bits 8, 7, 6, 2, 1 and 0 respectively.
pub const fn native_modifier_bits(modifiers: u32) -> u32 {
    (16 * (modifiers & 0x10))
        | (((modifiers >> 6) & 4)
            | ((modifiers >> 8) & 2)
            | ((modifiers >> 10) & 1)
            | (modifiers & 0x40)
            | (4 * (modifiers & 0x20)))
}

fn opcode_report(opcode: u32) -> [u8; DRIVER_INPUT_BYTES] {
    let mut bytes = [0; DRIVER_INPUT_BYTES];
    bytes[4..8].copy_from_slice(&opcode.to_le_bytes());
    bytes
}

pub fn encode_keyboard(input: KeyboardInput) -> NativeInputReport {
    let mut bytes = opcode_report(1);
    bytes[8..10].copy_from_slice(&input.unit_id.to_le_bytes());
    bytes[10..12].copy_from_slice(&input.scancode.to_le_bytes());
    bytes[12..14].copy_from_slice(&input.flag.to_le_bytes());
    bytes[14..16].copy_from_slice(&input.reserved.to_le_bytes());
    bytes[16..20].copy_from_slice(&native_modifier_bits(input.modifiers).to_le_bytes());
    NativeInputReport::from_native_input(bytes)
}

pub fn encode_mouse(input: MouseInput) -> NativeInputReport {
    let mut bytes = opcode_report(2);
    // SendInput copies the native MappingDataMouseWin bytes +56..+71 to
    // packet +8..+23, then native +72 (Y movement) to packet +24.
    bytes[8..10].copy_from_slice(&input.unit_id.to_le_bytes());
    bytes[10..12].copy_from_slice(&input.flags.to_le_bytes());
    bytes[12..14].copy_from_slice(&input.id.to_le_bytes());
    bytes[14..16].copy_from_slice(&input.data.to_le_bytes());
    bytes[16..20].copy_from_slice(&input.raw_buttons.to_le_bytes());
    bytes[20..24].copy_from_slice(&input.movement[0].to_le_bytes());
    bytes[24..28].copy_from_slice(&input.movement[1].to_le_bytes());
    bytes[28..32].copy_from_slice(&native_modifier_bits(input.modifiers).to_le_bytes());
    NativeInputReport::from_native_input(bytes)
}

pub fn encode_display(input: DisplayInput) -> NativeInputReport {
    let mut bytes = opcode_report(10);
    bytes[8..10].copy_from_slice(&input.payload_word().to_le_bytes());
    NativeInputReport::from_native_input(bytes)
}

pub fn encode_native_input(input: NativeInput) -> NativeInputReport {
    match input {
        NativeInput::Keyboard(input) => encode_keyboard(input),
        NativeInput::Mouse(input) => encode_mouse(input),
        NativeInput::Display(input) => encode_display(input),
    }
}

/// The native handler queues a second keyboard object for the E1 scan
/// sequence. It uses scan `0x45`, flag `original_flag & 1`, and feeds the
/// already converted modifier integer through the same conversion again.
/// This is a packet plan. Execution queues the continuation only when the
/// primary request was accepted, before awaiting its final completion.
pub fn encode_keyboard_sequence(input: KeyboardInput) -> Vec<NativeInputReport> {
    let mut reports = vec![encode_keyboard(input)];
    if let Some(continuation) = input.native_continuation() {
        reports.push(encode_keyboard(continuation));
    }
    reports
}

pub fn display_from_source_id(id: u32) -> Result<DisplayInput, NativeInputError> {
    match id {
        3 => Ok(DisplayInput::SourceId3),
        4 => Ok(DisplayInput::SourceId4),
        5 => Ok(DisplayInput::SourceId5),
        other => Err(NativeInputError::UnsupportedDisplaySource(other)),
    }
}

/// Decoding exposes the actual converted driver mask. The transform loses
/// unrelated source bits, so it cannot reconstruct a source modifier integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodedNativeInput {
    Keyboard {
        unit_id: u16,
        scancode: u16,
        flag: u16,
        reserved: u16,
        driver_modifiers: u32,
    },
    Mouse {
        unit_id: u16,
        flags: u16,
        id: u16,
        data: u16,
        raw_buttons: u32,
        movement: [i32; 2],
        driver_modifiers: u32,
    },
    Display(DisplayInput),
}

pub fn decode_native_input(bytes: &[u8]) -> Result<DecodedNativeInput, NativeInputError> {
    if bytes.len() != DRIVER_INPUT_BYTES {
        return Err(NativeInputError::InvalidPacketLength(bytes.len()));
    }
    fn check_zero(bytes: &[u8], start: usize, end: usize) -> Result<(), NativeInputError> {
        if let Some(offset) = (start..end).find(|&index| bytes[index] != 0) {
            Err(NativeInputError::NonzeroReserved { offset })
        } else {
            Ok(())
        }
    }
    fn word(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
    }
    fn dword(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }
    fn modifiers(bytes: &[u8], offset: usize) -> Result<u32, NativeInputError> {
        let mask = dword(bytes, offset);
        if mask & !0x1c7 != 0 {
            Err(NativeInputError::InvalidDriverModifiers(mask))
        } else {
            Ok(mask)
        }
    }
    check_zero(bytes, 0, 4)?;
    match dword(bytes, 4) {
        1 => {
            check_zero(bytes, 20, 32)?;
            Ok(DecodedNativeInput::Keyboard {
                unit_id: word(bytes, 8),
                scancode: word(bytes, 10),
                flag: word(bytes, 12),
                reserved: word(bytes, 14),
                driver_modifiers: modifiers(bytes, 16)?,
            })
        }
        2 => Ok(DecodedNativeInput::Mouse {
            unit_id: word(bytes, 8),
            flags: word(bytes, 10),
            id: word(bytes, 12),
            data: word(bytes, 14),
            raw_buttons: dword(bytes, 16),
            movement: [dword(bytes, 20) as i32, dword(bytes, 24) as i32],
            driver_modifiers: modifiers(bytes, 28)?,
        }),
        10 => {
            check_zero(bytes, 10, 32)?;
            let input = match word(bytes, 8) {
                112 => DisplayInput::SourceId3,
                111 => DisplayInput::SourceId4,
                0 => DisplayInput::SourceId5,
                other => return Err(NativeInputError::UnsupportedDisplayPayload(other)),
            };
            Ok(DecodedNativeInput::Display(input))
        }
        other => Err(NativeInputError::UnsupportedOpcode(other)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputReportOrigin {
    Primary { index: usize },
    E1Continuation { primary_index: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputReportExecution {
    pub input: NativeInput,
    pub origin: InputReportOrigin,
    pub report: NativeInputReport,
    pub delivery: ReportDelivery,
    pub observation_errors: Vec<DriverOutputError>,
    /// Only failed primary requests own the native failure callback. Driver
    /// E1 continuations have an empty callback and never trigger this fallback.
    pub os_fallback: Option<Result<OsInputExecution, OsInputError>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeInputExecution {
    pub reports: Vec<InputReportExecution>,
}

impl NativeInputExecution {
    /// Submit in caller order, with the native E1 continuation immediately
    /// after queue acceptance of its primary. Every result remains visible.
    pub fn submit(
        transport: &mut impl MappingOutputTransport,
        inputs: &[NativeInput],
        wait: Duration,
    ) -> Self {
        let mut execution = Self {
            reports: Vec::new(),
        };
        for (index, &input) in inputs.iter().enumerate() {
            let accepted =
                execution.submit_one(transport, input, InputReportOrigin::Primary { index });
            if accepted {
                if let NativeInput::Keyboard(keyboard) = input {
                    if let Some(continuation) = keyboard.native_continuation() {
                        execution.submit_one(
                            transport,
                            NativeInput::Keyboard(continuation),
                            InputReportOrigin::E1Continuation {
                                primary_index: index,
                            },
                        );
                    }
                }
            }
        }
        execution.poll(transport, wait);
        execution
    }

    fn submit_one(
        &mut self,
        transport: &mut impl MappingOutputTransport,
        input: NativeInput,
        origin: InputReportOrigin,
    ) -> bool {
        let report = encode_native_input(input);
        let (accepted, delivery) = match transport.submit(report) {
            Ok(id) => (true, ReportDelivery::Pending(id)),
            Err(error) => (false, ReportDelivery::Failed(error)),
        };
        self.reports.push(InputReportExecution {
            input,
            origin,
            report,
            delivery,
            observation_errors: Vec::new(),
            os_fallback: None,
        });
        accepted
    }

    pub fn is_pending(&self) -> bool {
        self.reports
            .iter()
            .any(|report| matches!(report.delivery, ReportDelivery::Pending(_)))
    }

    pub fn driver_confirmed(&self) -> bool {
        !self.reports.is_empty()
            && self
                .reports
                .iter()
                .all(|report| report.delivery == ReportDelivery::Completed)
    }

    /// The original native callback belongs to the primary request. The E1
    /// request uses a separate empty callback; its outcome is still recorded.
    pub fn primary_requests_confirmed(&self) -> bool {
        !self.reports.is_empty()
            && self
                .reports
                .iter()
                .filter(|report| matches!(report.origin, InputReportOrigin::Primary { .. }))
                .all(|report| report.delivery == ReportDelivery::Completed)
    }

    pub fn poll(&mut self, transport: &mut impl MappingOutputTransport, wait: Duration) {
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

    pub fn cancel_pending(&mut self, transport: &mut impl MappingOutputTransport, wait: Duration) {
        for report in &mut self.reports {
            if let ReportDelivery::Pending(id) = report.delivery {
                if let Err(error) = transport.cancel(id) {
                    report.observation_errors.push(error);
                }
            }
        }
        self.poll(transport, wait);
    }

    /// Execute RVA 0xA3910's failure callback for each newly failed primary.
    /// A pending request or failed completion observation is not a failure
    /// callback. Each fallback is attempted once and cannot alter the original
    /// driver receipt, even when the OS accepts all of its input events.
    pub fn fallback_failed_primaries(&mut self, transport: &mut impl OsInputTransport) {
        for report in &mut self.reports {
            if matches!(report.origin, InputReportOrigin::Primary { .. })
                && matches!(report.delivery, ReportDelivery::Failed(_))
                && report.os_fallback.is_none()
            {
                report.os_fallback = Some(OsInputExecution::submit(transport, report.input));
            }
        }
    }

    pub fn submit_with_os_fallback(
        driver: &mut impl MappingOutputTransport,
        os: &mut impl OsInputTransport,
        inputs: &[NativeInput],
        wait: Duration,
    ) -> Self {
        let mut execution = Self::submit(driver, inputs, wait);
        execution.fallback_failed_primaries(os);
        execution
    }

    pub fn poll_with_os_fallback(
        &mut self,
        driver: &mut impl MappingOutputTransport,
        os: &mut impl OsInputTransport,
        wait: Duration,
    ) {
        self.poll(driver, wait);
        self.fallback_failed_primaries(os);
    }
}

/// Platform-independent description of one Win32 INPUT. The native fallback
/// deliberately drops driver unit/reserved/modifier fields and mouse movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsInput {
    Keyboard { scancode: u16, flags: u32 },
    Mouse { flags: u32, data: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OsInputPlan {
    pub primary: OsInput,
    pub continuation: Option<OsInput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsInputError {
    UnsupportedPlatform,
    UnsupportedDisplayInput,
    UnsupportedMouseId(u16),
    /// Win32 SendInput must return exactly one. GetLastError is retained as an
    /// observation; Windows may leave it unchanged for input blocked by UIPI.
    SendInput {
        inserted: u32,
        win32_code: u32,
    },
}

impl fmt::Display for OsInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => write!(f, "native OS input fallback requires Windows"),
            Self::UnsupportedDisplayInput => write!(
                f,
                "native OS input fallback does not support display inputs"
            ),
            Self::UnsupportedMouseId(id) => {
                write!(f, "native OS input fallback does not support mouse id {id}")
            }
            Self::SendInput {
                inserted,
                win32_code,
            } => write!(
                f,
                "SendInput inserted {inserted} events instead of one (Win32 observation {win32_code})"
            ),
        }
    }
}

impl std::error::Error for OsInputError {}

/// Exact input conversion at RVA 0x11C510. The OS continuation runs only when
/// the primary SendInput call returned one; unlike the driver continuation,
/// its result contributes to the fallback's return value.
pub fn plan_os_input(input: NativeInput) -> Result<OsInputPlan, OsInputError> {
    match input {
        NativeInput::Keyboard(keyboard) => {
            let flags =
                8 | u32::from(keyboard.flag & 6 != 0) | (2 * u32::from(keyboard.is_released()));
            Ok(OsInputPlan {
                primary: OsInput::Keyboard {
                    scancode: keyboard.scancode,
                    flags,
                },
                continuation: (keyboard.scancode == 29 && keyboard.flag & 4 != 0).then_some(
                    OsInput::Keyboard {
                        scancode: 69,
                        flags: 8 | (2 * u32::from(keyboard.is_released())),
                    },
                ),
            })
        }
        NativeInput::Mouse(mouse) => {
            let (flags, data) = match mouse.id {
                1 => (2, 0),
                2 => (4, 0),
                4 => (8, 0),
                8 => (16, 0),
                16 => (32, 0),
                32 => (64, 0),
                64 => (128, 1),
                128 => (256, 1),
                256 => (128, 2),
                512 => (256, 2),
                1024 | 2048 => (
                    if mouse.id == 1024 { 2048 } else { 4096 },
                    if mouse.data == 120 { 120 } else { (-120i32) as u32 },
                ),
                other => return Err(OsInputError::UnsupportedMouseId(other)),
            };
            Ok(OsInputPlan {
                primary: OsInput::Mouse { flags, data },
                continuation: None,
            })
        }
        NativeInput::Display(_) => Err(OsInputError::UnsupportedDisplayInput),
    }
}

pub trait OsInputTransport {
    /// Return success only after the operating system accepted exactly one
    /// event. This is OS acceptance, not a filter-driver completion or evidence
    /// that a focused application acted on the event.
    fn send_one(&mut self, input: OsInput) -> Result<(), OsInputError>;
}

#[derive(Default)]
pub struct UnsupportedOsInputTransport;

impl OsInputTransport for UnsupportedOsInputTransport {
    fn send_one(&mut self, _input: OsInput) -> Result<(), OsInputError> {
        Err(OsInputError::UnsupportedPlatform)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OsInputReportExecution {
    pub input: OsInput,
    pub result: Result<(), OsInputError>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OsInputExecution {
    pub reports: Vec<OsInputReportExecution>,
}

impl OsInputExecution {
    pub fn submit(
        transport: &mut impl OsInputTransport,
        input: NativeInput,
    ) -> Result<Self, OsInputError> {
        let plan = plan_os_input(input)?;
        let result = transport.send_one(plan.primary);
        let mut reports = vec![OsInputReportExecution {
            input: plan.primary,
            result,
        }];
        if result.is_ok() {
            if let Some(continuation) = plan.continuation {
                reports.push(OsInputReportExecution {
                    input: continuation,
                    result: transport.send_one(continuation),
                });
            }
        }
        Ok(Self { reports })
    }

    pub fn os_accepted(&self) -> bool {
        !self.reports.is_empty() && self.reports.iter().all(|report| report.result.is_ok())
    }
}

#[cfg(test)]
#[path = "mapping_native_input_tests.rs"]
mod tests;

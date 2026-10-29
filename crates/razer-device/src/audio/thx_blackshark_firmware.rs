//! Current six-owner BlackShark/Valkyrie firmware EQ and local preset restore.
//!
//! AudioValkyrie -> AudioBillieT1Wired general reports use `hid.write`/`hid.read`,
//! report 2, transaction 0x60, and the source sign-magnitude band encoding.
//! Platform I/O is supplied through the transport; this module loads no vendor code.
//!
//! Rust boundary policy is separate from observed source behavior: descriptor
//! lengths and retained identity are checked, all input loops have a finite
//! budget, and scheduling/validation are injected. The source checks no reply
//! checksum, and this module does not invent one. Source batches do not stop or
//! roll back after failure; acknowledged partial writes are retained explicitly.

use crate::backend::{FeatureTransport, HidNode};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BLACKSHARK_REPORT_ID: u8 = 2;
pub const BLACKSHARK_REPORT_BYTES: usize = 64;
pub const BLACKSHARK_TRANSACTION: u8 = 0x60;
pub const BLACKSHARK_BAND_FREQUENCIES_HZ: [u32; 10] =
    [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
/// Default source maxRetryOut is 20, reduced elsewhere to at most 5; this
/// family's output runner explicitly uses min(maxRetryOut, 3).
pub const BLACKSHARK_OUTPUT_ATTEMPTS: usize = 3;
/// Rust policy: count every input packet, including invalid nonempty packets.
/// The source decrements its five-read counter only for empty packets.
pub const BLACKSHARK_INPUT_BUDGET: usize = 5;
/// Rust policy for a cancellable interrupt read; not a vendor timeout claim.
pub const BLACKSHARK_INPUT_TIMEOUT_MS: u32 = 50;

#[cfg(test)]
#[path = "thx_blackshark_firmware_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlackSharkFirmwareConnection {
    Wired,
    Dongle,
}

/// Only these six owners and their current connected USB PIDs are audited.
/// A Bluetooth or unrelated collection is not promoted to this USB protocol.
pub fn blackshark_firmware_route(
    owner_product_id: u32,
    connected_product_id: u16,
) -> Result<BlackSharkFirmwareConnection> {
    let dongle = match owner_product_id {
        1398 => 1399,
        1401 => 1402,
        2638 => 2645,
        2641 => 2648,
        4124 => 4123,
        // DeviceInfo.unSupportWired=true, despite owner/dongle PID equality.
        4126 => 4126,
        _ => anyhow::bail!("BlackShark firmware owner has no current source audit"),
    };
    if connected_product_id == dongle {
        Ok(BlackSharkFirmwareConnection::Dongle)
    } else {
        ensure!(
            owner_product_id != 4126 && u32::from(connected_product_id) == owner_product_id,
            "Connected PID is not this BlackShark owner's wired/dongle USB PID"
        );
        Ok(BlackSharkFirmwareConnection::Wired)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackSharkFirmwareIdentity {
    pub owner_product_id: u32,
    /// Raw collection path, connected PID, interface and observed serial are
    /// retained together; selecting the first VID/PID match is insufficient.
    pub node: HidNode,
    pub device_container_id: String,
    pub generation: u64,
}
impl BlackSharkFirmwareIdentity {
    pub fn new(
        owner_product_id: u32,
        node: HidNode,
        device_container_id: impl Into<String>,
        generation: u64,
    ) -> Result<Self> {
        let identity = Self {
            owner_product_id,
            node,
            device_container_id: device_container_id.into(),
            generation,
        };
        identity.validate()?;
        Ok(identity)
    }
    pub fn validate(&self) -> Result<()> {
        blackshark_firmware_route(self.owner_product_id, self.node.product_id)?;
        ensure!(
            self.node.vendor_id == 0x1532,
            "BlackShark HID VID differs from current source"
        );
        ensure!(
            self.node.interface_number == 5,
            "BlackShark HID is not claimInterface 5"
        );
        ensure!(
            !self.node.path.is_empty(),
            "BlackShark HID collection path is empty"
        );
        ensure!(
            !self.device_container_id.is_empty() && !self.device_container_id.contains('\0'),
            "BlackShark deviceContainerId is empty or contains NUL"
        );
        if let Some(serial) = &self.node.serial_number {
            ensure!(
                !serial.is_empty() && !serial.contains('\0'),
                "Observed BlackShark serial is invalid"
            );
        }
        Ok(())
    }
    pub fn connection(&self) -> Result<BlackSharkFirmwareConnection> {
        blackshark_firmware_route(self.owner_product_id, self.node.product_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlackSharkLocalEqPreset {
    pub index: u8,
    pub voice_prompt_index: u8,
    pub band_values: [i16; 10],
    pub enable_state: bool,
    pub current_active: bool,
    pub cloud_equalizer_id: u8,
    pub custom: bool,
}
impl BlackSharkLocalEqPreset {
    pub fn parse(value: &Value) -> Result<Self> {
        let preset: Self = serde_json::from_value(value.clone())
            .context("Local cloudAudioEqualizer preset schema is invalid")?;
        preset.validate()?;
        Ok(preset)
    }
    pub fn validate(&self) -> Result<()> {
        for &band in &self.band_values {
            encode_blackshark_band(band)?;
        }
        Ok(())
    }
    pub fn order(&self) -> BlackSharkEqOrder {
        BlackSharkEqOrder {
            index: self.index,
            voice_prompt_index: self.voice_prompt_index,
            enabled: self.enable_state,
            active: self.current_active,
            cloud_equalizer_id: self.cloud_equalizer_id,
            custom: self.custom,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackSharkEqOrder {
    pub index: u8,
    pub voice_prompt_index: u8,
    pub enabled: bool,
    pub active: bool,
    pub cloud_equalizer_id: u8,
    pub custom: bool,
}

/// Source negatives are 128+abs(value), not signed two's-complement bytes.
/// Restrict input to exactly representable integer levels as Rust policy;
/// wrapping arbitrary JS numbers into Uint8Array would lose the user's value.
pub fn encode_blackshark_band(level: i16) -> Result<u8> {
    ensure!(
        (-127..=128).contains(&level),
        "BlackShark EQ level cannot round-trip through a byte"
    );
    Ok(if level < 0 {
        (128 - level) as u8
    } else {
        level as u8
    })
}
pub fn decode_blackshark_band(value: u8) -> i16 {
    if value > 128 {
        128 - i16::from(value)
    } else {
        i16::from(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlackSharkFirmwareCommand {
    GetFirmwareVersions,
    FactoryReset1398,
    GetMicStatus1398,
    GetRollerFunction,
    SetRollerFunction(u8),
    SetPresetEqEnabled(bool),
    GetPresetEqStatus,
    BatchStart,
    BatchStop,
    SetBands { index: u8, bands: [i16; 10] },
    GetBands { index: u8 },
    SetOrder(BlackSharkEqOrder),
    GetOrder { index: u8 },
}
impl BlackSharkFirmwareCommand {
    pub fn command_header(&self) -> [u8; 3] {
        match self {
            Self::GetFirmwareVersions => [2, 0, 0],
            Self::FactoryReset1398 => [239, 0, 1],
            Self::GetMicStatus1398 => [85, 0, 0],
            Self::GetRollerFunction => [0x6a, 0, 0],
            Self::SetRollerFunction(_) => [0xea, 0, 1],
            Self::SetPresetEqEnabled(_) => [0x9e, 0, 1],
            Self::GetPresetEqStatus => [0x1e, 0, 0],
            Self::BatchStart | Self::BatchStop => [0xe1, 0, 1],
            Self::SetBands { .. } => [0x95, 0, 11],
            Self::GetBands { .. } => [0x15, 0, 1],
            Self::SetOrder(_) => [0xe0, 0, 6],
            Self::GetOrder { .. } => [0x60, 0, 1],
        }
    }
    pub fn expected_response(&self) -> [u8; 3] {
        match self {
            Self::GetFirmwareVersions => [2, 1, 24],
            Self::FactoryReset1398 => [239, 1, 1],
            Self::GetMicStatus1398 => [85, 1, 1],
            Self::GetRollerFunction => [0x6a, 1, 1],
            Self::SetRollerFunction(_) => [0xea, 1, 1],
            Self::SetPresetEqEnabled(_) => [0x9e, 1, 1],
            Self::GetPresetEqStatus => [0x1e, 1, 1],
            Self::BatchStart | Self::BatchStop => [0xe1, 1, 1],
            Self::SetBands { .. } => [0x95, 1, 1],
            Self::GetBands { .. } => [0x15, 1, 11],
            Self::SetOrder(_) => [0xe0, 1, 1],
            Self::GetOrder { .. } => [0x60, 1, 6],
        }
    }
    pub fn payload(&self) -> Result<Vec<u8>> {
        Ok(match self {
            Self::GetFirmwareVersions => vec![],
            Self::FactoryReset1398 => vec![0],
            Self::GetMicStatus1398 => vec![],
            Self::GetRollerFunction => vec![],
            Self::SetRollerFunction(index) => {
                ensure!(
                    matches!(index, 0..=3 | 47),
                    "Unknown current roller function"
                );
                vec![*index]
            }
            Self::SetPresetEqEnabled(enabled) => vec![u8::from(*enabled)],
            Self::GetPresetEqStatus => vec![],
            Self::BatchStart => vec![1],
            Self::BatchStop => vec![2],
            Self::SetBands { index, bands } => {
                let mut payload = vec![*index];
                for &band in bands {
                    payload.push(encode_blackshark_band(band)?);
                }
                payload
            }
            Self::GetBands { index } | Self::GetOrder { index } => vec![*index],
            Self::SetOrder(order) => vec![
                order.index,
                order.voice_prompt_index,
                u8::from(order.enabled),
                u8::from(order.active),
                order.cloud_equalizer_id,
                u8::from(order.custom),
            ],
        })
    }
}

pub fn pack_blackshark_general_report(
    command: &BlackSharkFirmwareCommand,
    connection: BlackSharkFirmwareConnection,
    is_dongle_command: bool,
) -> Result<[u8; BLACKSHARK_REPORT_BYTES]> {
    let header = command.command_header();
    let payload = command.payload()?;
    ensure!(
        payload.len() == usize::from(header[2]),
        "BlackShark command payload length is inconsistent"
    );
    let mut report = [0u8; BLACKSHARK_REPORT_BYTES];
    report[0] = BLACKSHARK_REPORT_ID;
    report[2] = BLACKSHARK_TRANSACTION;
    report[6] = (header.len() + payload.len() + 1) as u8;
    report[9] = if connection == BlackSharkFirmwareConnection::Dongle && !is_dongle_command {
        0x80
    } else {
        0
    };
    report[10..13].copy_from_slice(&header);
    report[13..13 + payload.len()].copy_from_slice(&payload);
    report[62] = report[..62].iter().fold(0, |sum, byte| sum ^ byte);
    Ok(report)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackSharkFirmwareReply {
    pub status: u8,
    pub resend_requested: bool,
    pub header: Vec<u8>,
    pub data: Vec<u8>,
}
impl BlackSharkFirmwareReply {
    pub fn source_success(&self) -> bool {
        self.status == 2
    }
}
pub fn decode_blackshark_general_reply(
    report: &[u8],
    expected_response: [u8; 3],
) -> Result<BlackSharkFirmwareReply> {
    ensure!(
        report.len() == BLACKSHARK_REPORT_BYTES,
        "BlackShark input length is not 64"
    );
    ensure!(
        report[0] == BLACKSHARK_REPORT_ID,
        "BlackShark input Report ID differs"
    );
    ensure!(
        report[2] == BLACKSHARK_TRANSACTION,
        "BlackShark input transaction differs"
    );
    ensure!(
        report[10..12] == expected_response[..2],
        "BlackShark input command header differs"
    );
    let data_end = 13 + usize::from(expected_response[2]);
    ensure!(
        data_end <= 62,
        "BlackShark expected response extends past the payload"
    );
    // Source deliberately does not compare report[12], report[6], routing byte
    // or checksum. Do not reject valid-source replies by inventing validation.
    Ok(BlackSharkFirmwareReply {
        status: report[1],
        resend_requested: matches!(report[1], 1 | 3),
        header: report[..13].to_vec(),
        data: report[13..data_end].to_vec(),
    })
}

pub fn decode_blackshark_eq_bands(data: &[u8], requested_index: u8) -> Result<[i16; 10]> {
    ensure!(
        data.len() == 11,
        "BlackShark EQ bands reply is not eleven bytes"
    );
    // Extra Rust observation guard: the source exposes the returned index.
    ensure!(
        data[0] == requested_index,
        "BlackShark EQ bands reply index differs"
    );
    let mut bands = [0; 10];
    for (band, &byte) in bands.iter_mut().zip(&data[1..]) {
        *band = decode_blackshark_band(byte);
    }
    Ok(bands)
}
pub fn decode_blackshark_eq_order(data: &[u8], requested_index: u8) -> Result<BlackSharkEqOrder> {
    ensure!(
        data.len() == 6,
        "BlackShark EQ order reply is not six bytes"
    );
    ensure!(
        data[0] == requested_index,
        "BlackShark EQ order reply index differs"
    );
    Ok(BlackSharkEqOrder {
        index: data[0],
        voice_prompt_index: data[1],
        enabled: data[2] == 1,
        active: data[3] == 1,
        cloud_equalizer_id: data[4],
        custom: data[5] == 1,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlackSharkFirmwareFailureKind {
    InvalidRequest,
    Validation,
    Scheduling,
    Transport,
    DeviceRejected,
    InputExhausted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackSharkFirmwareFailure {
    pub kind: BlackSharkFirmwareFailureKind,
    pub message: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackSharkFirmwareAttempt {
    pub output: Vec<u8>,
    pub output_attempted: bool,
    pub output_completed: bool,
    /// Retain every actual returned packet, including malformed packets.
    pub inputs: Vec<Vec<u8>>,
    pub rejected_inputs: Vec<String>,
    pub reply: Option<BlackSharkFirmwareReply>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackSharkFirmwareCommandProgress {
    pub command: BlackSharkFirmwareCommand,
    pub identity: BlackSharkFirmwareIdentity,
    pub attempts: Vec<BlackSharkFirmwareAttempt>,
    /// True only after a success envelope AND current identity/cancel guards.
    pub completed: bool,
    pub failure: Option<BlackSharkFirmwareFailure>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlackSharkMicStatus {
    Unmuted,
    Muted,
    Unknown(u8),
}
impl BlackSharkFirmwareCommandProgress {
    pub fn observed_mic_status(&self) -> Option<BlackSharkMicStatus> {
        if self.command != BlackSharkFirmwareCommand::GetMicStatus1398 || !self.succeeded() {
            return None;
        }
        match self.reply()?.data.as_slice() {
            [0] => Some(BlackSharkMicStatus::Unmuted),
            [1] => Some(BlackSharkMicStatus::Muted),
            [value] => Some(BlackSharkMicStatus::Unknown(*value)),
            _ => None,
        }
    }
    /// Current hasMultipleFwVersions selects the numerically greatest nonzero
    /// four-byte version. Source fallback 1.0.0.0 is not a device observation.
    pub fn observed_firmware_version(&self) -> Option<String> {
        if self.command != BlackSharkFirmwareCommand::GetFirmwareVersions || !self.succeeded() {
            return None;
        }
        let data = &self.reply()?.data;
        if data.len() != 24 {
            return None;
        }
        let version = data
            .chunks_exact(4)
            .filter(|version| version.iter().any(|byte| *byte != 0))
            .max()?;
        Some(format!(
            "{}.{}.{}.{}",
            version[0], version[1], version[2], version[3]
        ))
    }
    fn new(command: BlackSharkFirmwareCommand, identity: BlackSharkFirmwareIdentity) -> Self {
        Self {
            command,
            identity,
            attempts: vec![],
            completed: false,
            failure: None,
        }
    }
    pub fn succeeded(&self) -> bool {
        self.completed && self.failure.is_none()
    }
    pub fn source_status(&self) -> &'static str {
        if self.succeeded() { "SUCCESS" } else { "FAIL" }
    }
    /// Acknowledgement may be observed before a late cancel/disconnect. It is
    /// retained even when the operation is not adopted as current successful state.
    pub fn acknowledged(&self) -> bool {
        self.attempts.iter().any(|attempt| {
            attempt
                .reply
                .as_ref()
                .is_some_and(BlackSharkFirmwareReply::source_success)
        })
    }
    pub fn reply(&self) -> Option<&BlackSharkFirmwareReply> {
        self.attempts
            .iter()
            .rev()
            .find_map(|attempt| attempt.reply.as_ref())
    }
    fn fail(&mut self, kind: BlackSharkFirmwareFailureKind, error: impl std::fmt::Display) {
        self.failure = Some(BlackSharkFirmwareFailure {
            kind,
            message: error.to_string(),
        });
    }
}

pub struct BlackSharkFirmwareSession {
    identity: BlackSharkFirmwareIdentity,
    transport: Box<dyn FeatureTransport>,
    /// Exact constructor state; ordinary send and explicit dongle send reset
    /// false afterward, including errors. The first ordinary command on a newly
    /// constructed dongle therefore has byte9==0, not a guessed 0x80.
    is_dongle_command: bool,
    last_command: Option<BlackSharkFirmwareCommandProgress>,
}
impl BlackSharkFirmwareSession {
    pub fn new(
        identity: BlackSharkFirmwareIdentity,
        transport: Box<dyn FeatureTransport>,
    ) -> Result<Self> {
        identity.validate()?;
        let lengths = transport.report_lengths()?;
        ensure!(
            lengths.input.get(&2) == Some(&64) && lengths.output.get(&2) == Some(&64),
            "BlackShark requires descriptor-observed report 2 input/output lengths of exactly 64"
        );
        Ok(Self {
            identity,
            transport,
            is_dongle_command: true,
            last_command: None,
        })
    }
    pub fn identity(&self) -> &BlackSharkFirmwareIdentity {
        &self.identity
    }
    pub fn transport(&self) -> &dyn FeatureTransport {
        &*self.transport
    }
    pub fn last_command(&self) -> Option<&BlackSharkFirmwareCommandProgress> {
        self.last_command.as_ref()
    }

    fn run_command(
        &self,
        command: BlackSharkFirmwareCommand,
        is_dongle_command: bool,
        validate: &mut impl FnMut(&BlackSharkFirmwareIdentity) -> Result<()>,
        wait: &mut impl FnMut(u64) -> Result<()>,
    ) -> BlackSharkFirmwareCommandProgress {
        use BlackSharkFirmwareFailureKind::*;
        let mut progress = BlackSharkFirmwareCommandProgress::new(command, self.identity.clone());
        if matches!(
            progress.command,
            BlackSharkFirmwareCommand::FactoryReset1398
                | BlackSharkFirmwareCommand::GetMicStatus1398
        ) && self.identity.owner_product_id != crate::blackshark_reset::PRODUCT_ID
        {
            progress.fail(
                InvalidRequest,
                "Help reset firmware commands are audited only for 1398",
            );
            return progress;
        }
        let connection = match self.identity.connection() {
            Ok(connection) => connection,
            Err(error) => {
                progress.fail(InvalidRequest, error);
                return progress;
            }
        };
        let report = match pack_blackshark_general_report(
            &progress.command,
            connection,
            is_dongle_command,
        ) {
            Ok(report) => report,
            Err(error) => {
                progress.fail(InvalidRequest, error);
                return progress;
            }
        };
        let expected = progress.command.expected_response();
        let validation = std::cell::RefCell::new(validate);
        let check = || validation.borrow_mut()(&self.identity);
        // Fixed full interval is conservative when the caller has already
        // waited; no wall-clock dependency is introduced into shared logic.
        if let Err(error) = check() {
            progress.fail(Validation, error);
            return progress;
        }
        if let Err(error) = wait(10) {
            progress.fail(Scheduling, error);
            return progress;
        }
        if let Err(error) = check() {
            progress.fail(Validation, error);
            return progress;
        }
        let delay = if connection == BlackSharkFirmwareConnection::Wired {
            5
        } else {
            30
        };
        let lease = match self.transport.begin_exchange_validated(
            crate::backend::ExchangeOptions::plain(),
            std::rc::Rc::new(&check),
        ) {
            Ok(lease) => lease,
            Err(error) => {
                progress.fail(Transport, error);
                return progress;
            }
        };
        let mut progress = (|| {
            let transport = lease.transport();
            for _ in 0..BLACKSHARK_OUTPUT_ATTEMPTS {
                if let Err(error) = check() {
                    progress.fail(Validation, error);
                    return progress;
                }
                let mut attempt = BlackSharkFirmwareAttempt {
                    output: report.to_vec(),
                    output_attempted: true,
                    ..Default::default()
                };
                let result = transport.write_output(&report);
                attempt.output_completed = result.is_ok();
                progress.attempts.push(attempt);
                if let Err(error) = result {
                    progress.fail(Transport, error);
                    return progress;
                }
                if let Err(error) = check() {
                    progress.fail(Validation, error);
                    return progress;
                }
                if let Err(error) = wait(delay) {
                    progress.fail(Scheduling, error);
                    return progress;
                }
                for input_ordinal in 0..BLACKSHARK_INPUT_BUDGET {
                    if let Err(error) = check() {
                        progress.fail(Validation, error);
                        return progress;
                    }
                    if input_ordinal > 0 {
                        // Rust also spaces invalid-nonempty reads; source only
                        // applies this delay once an empty read reduced its counter.
                        if let Err(error) = wait(delay) {
                            progress.fail(Scheduling, error);
                            return progress;
                        }
                        if let Err(error) = check() {
                            progress.fail(Validation, error);
                            return progress;
                        }
                    }
                    let mut input = [0u8; BLACKSHARK_REPORT_BYTES];
                    let count =
                        match transport.read_interrupt(&mut input, BLACKSHARK_INPUT_TIMEOUT_MS) {
                            Ok(count) if count <= input.len() => count,
                            Ok(count) => {
                                progress.fail(
                                    Transport,
                                    format!(
                                        "BlackShark backend returned impossible byte count {count}"
                                    ),
                                );
                                return progress;
                            }
                            Err(error) => {
                                progress.fail(Transport, error);
                                return progress;
                            }
                        };
                    let attempt = progress.attempts.last_mut().expect("attempt was pushed");
                    attempt.inputs.push(input[..count].to_vec());
                    let decoded = decode_blackshark_general_reply(&input[..count], expected);
                    if let Ok(reply) = &decoded {
                        attempt.reply = Some(reply.clone());
                    }
                    // Decode first to preserve a physical acknowledgement even if
                    // cancellation or identity changed while the read was pending.
                    if let Err(error) = check() {
                        progress.fail(Validation, error);
                        return progress;
                    }
                    match decoded {
                        Ok(reply) if reply.source_success() => {
                            progress.completed = true;
                            return progress;
                        }
                        Ok(_) => break,
                        Err(error) => attempt.rejected_inputs.push(error.to_string()),
                    }
                }
            }
            if let Some(reply) = progress.reply() {
                let status = reply.status;
                progress.fail(
                    DeviceRejected,
                    format!(
                        "BlackShark firmware reply status {status} after three output attempts"
                    ),
                );
            } else {
                progress.fail(
                    InputExhausted,
                    "BlackShark input budget exhausted after three output attempts",
                );
            }
            progress
        })();
        let released = match check() {
            Ok(()) => lease.finish(),
            Err(error) => {
                drop(lease);
                if let Some(failure) = &mut progress.failure {
                    failure.message.push_str(&format!(
                        "; HID exchange owner validation also failed: {error:#}"
                    ));
                } else {
                    progress.fail(Validation, error);
                }
                return progress;
            }
        };
        if let Err(error) = released {
            if let Some(failure) = &mut progress.failure {
                failure
                    .message
                    .push_str(&format!("; HID exchange release also failed: {error:#}"));
            } else {
                progress.fail(Transport, error);
            }
        }
        progress
    }

    pub fn exchange(
        &mut self,
        command: BlackSharkFirmwareCommand,
        mut validate: impl FnMut(&BlackSharkFirmwareIdentity) -> Result<()>,
        mut wait: impl FnMut(u64) -> Result<()>,
    ) -> BlackSharkFirmwareCommandProgress {
        let progress = self.run_command(command, self.is_dongle_command, &mut validate, &mut wait);
        self.is_dongle_command = false;
        self.last_command = Some(progress.clone());
        progress
    }
    pub fn exchange_to_dongle(
        &mut self,
        command: BlackSharkFirmwareCommand,
        mut validate: impl FnMut(&BlackSharkFirmwareIdentity) -> Result<()>,
        mut wait: impl FnMut(u64) -> Result<()>,
    ) -> BlackSharkFirmwareCommandProgress {
        let progress = self.run_command(command, true, &mut validate, &mut wait);
        self.is_dongle_command = false;
        self.last_command = Some(progress.clone());
        progress
    }
    pub fn set_preset_eq_enabled(
        &mut self,
        enabled: bool,
        validate: impl FnMut(&BlackSharkFirmwareIdentity) -> Result<()>,
        wait: impl FnMut(u64) -> Result<()>,
    ) -> BlackSharkFirmwareCommandProgress {
        self.exchange(
            BlackSharkFirmwareCommand::SetPresetEqEnabled(enabled),
            validate,
            wait,
        )
    }
    pub fn get_preset_eq_status(
        &mut self,
        validate: impl FnMut(&BlackSharkFirmwareIdentity) -> Result<()>,
        wait: impl FnMut(u64) -> Result<()>,
    ) -> BlackSharkFirmwareCommandProgress {
        self.exchange(BlackSharkFirmwareCommand::GetPresetEqStatus, validate, wait)
    }

    pub fn restore_local_eq(
        &mut self,
        presets: &[Value],
        selected_index: Option<&Value>,
        mut validate: impl FnMut(&BlackSharkFirmwareIdentity) -> Result<()>,
        mut wait: impl FnMut(u64) -> Result<()>,
    ) -> BlackSharkLocalEqRestoreProgress {
        let mut restore = BlackSharkLocalEqRestoreProgress {
            identity: self.identity.clone(),
            selected_index: selected_index.cloned(),
            supplied_indices: vec![],
            commands: vec![],
            batch_start_acknowledged: false,
            batch_stop_acknowledged: false,
            completed: false,
            failure: None,
        };
        let parsed = (|| -> Result<Vec<BlackSharkLocalEqPreset>> {
            // Source -1 (or omitted) selects all; other values use strict index
            // equality and the first match. Retain input order and duplicates.
            let selected = match selected_index {
                None => None,
                Some(value) if value.as_i64() == Some(-1) => None,
                Some(value) => Some(
                    value
                        .as_u64()
                        .and_then(|index| u8::try_from(index).ok())
                        .context("Selected local EQ index is not a USB byte or -1")?,
                ),
            };
            if let Some(index) = selected {
                let value = presets
                    .iter()
                    .find(|value| value["index"].as_u64() == Some(u64::from(index)))
                    .context("Selected local EQ preset is absent")?;
                Ok(vec![BlackSharkLocalEqPreset::parse(value)?])
            } else {
                presets.iter().map(BlackSharkLocalEqPreset::parse).collect()
            }
        })();
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                restore.failure = Some(BlackSharkFirmwareFailure {
                    kind: BlackSharkFirmwareFailureKind::InvalidRequest,
                    message: format!("{error:#}"),
                });
                return restore;
            }
        };
        restore.supplied_indices = parsed.iter().map(|preset| preset.index).collect();
        let mut commands = vec![BlackSharkFirmwareCommand::BatchStart];
        for preset in parsed {
            commands.push(BlackSharkFirmwareCommand::SetBands {
                index: preset.index,
                bands: preset.band_values,
            });
            commands.push(BlackSharkFirmwareCommand::SetOrder(preset.order()));
        }
        commands.push(BlackSharkFirmwareCommand::BatchStop);
        for command in commands {
            let is_start = command == BlackSharkFirmwareCommand::BatchStart;
            let is_stop = command == BlackSharkFirmwareCommand::BatchStop;
            let progress = self.exchange(command, &mut validate, &mut wait);
            if is_start {
                restore.batch_start_acknowledged = progress.acknowledged();
            }
            if is_stop {
                restore.batch_stop_acknowledged = progress.acknowledged();
            }
            let success = progress.succeeded();
            restore.failure = progress.failure.clone();
            restore.commands.push(progress);
            if !success {
                // Exactly the source catch/false path: no extra batch stop and
                // no rollback or successful persistence claim.
                return restore;
            }
        }
        restore.completed = true;
        restore
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlackSharkLocalEqRestoreProgress {
    pub identity: BlackSharkFirmwareIdentity,
    pub selected_index: Option<Value>,
    pub supplied_indices: Vec<u8>,
    pub commands: Vec<BlackSharkFirmwareCommandProgress>,
    pub batch_start_acknowledged: bool,
    pub batch_stop_acknowledged: bool,
    pub completed: bool,
    pub failure: Option<BlackSharkFirmwareFailure>,
}
impl BlackSharkLocalEqRestoreProgress {
    pub fn succeeded(&self) -> bool {
        self.completed && self.failure.is_none()
    }
    pub fn source_bool(&self) -> bool {
        self.succeeded()
    }
}

/// Current local built-ins are byte-identical for all six audited owners.
/// These are local defaults, not an observation of the attached device.
pub fn default_blackshark_local_eq_presets(
    owner_product_id: u32,
) -> Result<Vec<BlackSharkLocalEqPreset>> {
    ensure!(
        matches!(owner_product_id, 1398 | 1401 | 2638 | 2641 | 4124 | 4126),
        "No current BlackShark local preset defaults for this owner"
    );
    let bands = [
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [2, 2, 5, 5, 1, -1, 2, 3, 3, 3],
        [2, 2, 0, 0, 1, -1, -1, 3, 3, 3],
        [3, 3, 3, -1, -4, -4, 2, 3, 3, 3],
        [1, 1, -1, 0, 2, 0, 4, 4, 4, -3],
        [0, 0, 3, 3, 0, 0, 2, 3, 1, 1],
        [-4, -3, -1, 3, -2, -2, 4, 4, 4, 0],
        [-2, 0, 3, 3, 3, 0, 0, 3, 3, 0],
        [2, 2, 3, 3, -1, -2, 1, 4, 2, 2],
    ];
    let ids = [0, 1, 2, 3, 10, 11, 12, 13, 14];
    Ok(bands
        .into_iter()
        .enumerate()
        .map(|(index, band_values)| BlackSharkLocalEqPreset {
            index: index as u8,
            voice_prompt_index: index as u8,
            band_values,
            enable_state: index < 5,
            current_active: index == 0,
            cloud_equalizer_id: ids[index],
            custom: false,
        })
        .collect())
}

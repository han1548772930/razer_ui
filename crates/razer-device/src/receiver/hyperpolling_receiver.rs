//! PID179's mounted Uma receiver commands. Device mode is source-local state;
//! serial, edition and indicator commands use the physical primary channel.
use crate::{
    backend::{FeatureTransport, HidNode},
    receiver_capabilities::{ReceiverCapability, capability},
    receiver_feature_channel::{Channel, WriteProgress, exchange_channel},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const PRODUCT_ID: u16 = 179;
pub const SERIAL: [u8; 3] = [22, 0, 130];
pub const EDITION: [u8; 3] = [3, 0, 134];
pub const GET_INDICATOR: [u8; 3] = [1, 7, 144];
pub const SET_INDICATOR: [u8; 3] = [1, 7, 16];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Target {
    Collection(HidNode),
    WindowsContainer { path: String, container: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Command {
    Serial,
    Edition,
    Indicator,
    SetIndicator { status: u8 },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct SerialNumber {
    pub bytes: Vec<u8>,
}
impl SerialNumber {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
    /// Current84816.d filters all printable ASCII bytes, including after NUL.
    pub fn ascii(&self) -> String {
        self.bytes
            .iter()
            .copied()
            .filter(|byte| (32..127).contains(byte))
            .map(char::from)
            .collect()
    }
    /// The current runtime uses this sentinel only after a successful empty GET.
    pub fn source_identity(&self) -> String {
        let serial = self.ascii();
        if serial.is_empty() {
            "NOSERIALNUMBER".into()
        } else {
            serial
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Edition {
    pub keyboard_layout: u8,
    pub edition: u8,
    pub firmware_id: u8,
}
impl Edition {
    pub fn from_source_bytes([keyboard_layout, edition, firmware_id]: [u8; 3]) -> Self {
        Self {
            keyboard_layout,
            edition,
            firmware_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Submission {
    pub requested_status: u8,
    pub write_attempted: bool,
    pub write_sent: bool,
    pub acknowledged: bool,
    pub response_status: Option<u8>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Reading {
    Serial(SerialNumber),
    Edition(Edition),
    Indicator { status: Option<u8> },
    Submitted(Submission),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Observation {
    pub target: Target,
    pub reading: Reading,
}
impl Observation {
    pub fn new(target: Target, reading: Reading) -> Self {
        Self { target, reading }
    }
    pub fn validate_for(&self, target: &Target, command: Command) -> anyhow::Result<()> {
        ensure!(
            self.target == *target,
            "Uma reply belongs to another physical receiver"
        );
        ensure!(
            matches!(
                (&self.reading, command),
                (Reading::Serial(_), Command::Serial)
                    | (Reading::Edition(_), Command::Edition)
                    | (Reading::Indicator { .. }, Command::Indicator)
            ) || matches!((&self.reading, command), (Reading::Submitted(report), Command::SetIndicator { status }) if report.requested_status == status),
            "Uma reply belongs to another command"
        );
        Ok(())
    }
}

pub fn validate_node(node: &HidNode) -> anyhow::Result<()> {
    ensure!(
        node.vendor_id == 0x1532 && node.product_id == PRODUCT_ID && node.interface_number == 0,
        "Uma requires the exact current primary receiver collection"
    );
    Ok(())
}
pub fn source_capability() -> &'static ReceiverCapability {
    capability(PRODUCT_ID).expect("current Uma primary capability")
}
fn channel(cap: &ReceiverCapability) -> anyhow::Result<Channel> {
    ensure!(
        cap.product_id == PRODUCT_ID
            && cap.source_product_id == PRODUCT_ID
            && cap.vendor_id == 0x1532
            && cap.source_class == "rzDevice25LinkerUma"
            && cap.claim_interface == 0
            && cap.report_id == 0
            && cap.report_bytes == 91
            && cap.transaction_prefix == 224
            && cap.transaction_modulus == 31,
        "Uma capability differs from its mounted factory"
    );
    Ok(Channel {
        report_id: 0,
        get_report_id: 0,
        transaction_prefix: 224,
        transaction_modulus: 31,
        max_retry_in: cap.max_retry_in,
        max_retry_out: cap.max_retry_out,
        sleep_between_out_ms: cap.sleep_between_out_ms,
        sleep_between_out_in_ms: cap.sleep_between_out_in_ms,
        sleep_between_in_ms: cap.sleep_between_in_ms,
        ignore_set_device_mode_timeout_error: false,
    })
}

pub fn execute(
    device: &dyn FeatureTransport,
    cap: &ReceiverCapability,
    command: Command,
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Reading> {
    let cap = channel(cap)?;
    let mut progress = WriteProgress::default();
    let (header, payload) = match command {
        Command::Serial => (SERIAL, Vec::new()),
        Command::Edition => (EDITION, Vec::new()),
        Command::Indicator => (GET_INDICATOR, vec![0]),
        Command::SetIndicator { status } => (SET_INDICATOR, vec![status]),
    };
    let result = exchange_channel(
        device,
        &cap,
        header,
        &payload,
        transaction,
        validate,
        &mut progress,
    );
    if let Command::SetIndicator { status } = command {
        // A sent command and its real ACK remain visible if cancellation or a
        // transport failure follows. No getter or UI rollback is synthesized.
        return Ok(Reading::Submitted(Submission {
            requested_status: status,
            write_attempted: progress.attempted,
            write_sent: progress.sent,
            acknowledged: progress.acknowledged,
            response_status: result
                .as_ref()
                .ok()
                .and_then(|bytes| bytes.first().copied()),
            error: result.err().map(|error| format!("{error:#}")),
        }));
    }
    let data = result?;
    Ok(match command {
        Command::Serial => Reading::Serial(SerialNumber { bytes: data }),
        Command::Edition => {
            let fields = data.get(..3).context("Uma edition response is truncated")?;
            Reading::Edition(Edition {
                keyboard_layout: fields[0],
                edition: fields[1],
                firmware_id: fields[2],
            })
        }
        Command::Indicator => Reading::Indicator {
            status: data.first().copied(),
        },
        Command::SetIndicator { .. } => unreachable!(),
    })
}

#[cfg(test)]
#[path = "hyperpolling_receiver_tests.rs"]
mod tests;

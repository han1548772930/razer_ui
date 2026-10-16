//! Current 1352 rzDevice25Noa sound tasks. Physical frames are 65 bytes,
//! including Report ID 8. Profile persistence belongs to the caller; a setter
//! acknowledgement does not replace optimistic source UI state.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

pub const PRODUCT_ID: u32 = 1352;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "route", rename_all = "snake_case")]
pub enum EventTarget {
    Collection(crate::backend::HidNode),
    WindowsContainer(crate::device_reads::DeviceReadTarget),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notification {
    Input {
        source: u8,
    },
    Io {
        headphone: u8,
        usb: u8,
        aux: u8,
        optical: u8,
        bluetooth: u8,
    },
    Mode {
        mode: u8,
    },
    Output {
        state: u8,
    },
    Camera {
        locked: u8,
        direction: f32,
        distance: f32,
    },
}
pub fn notification(bytes: &[u8]) -> anyhow::Result<Option<Notification>> {
    if bytes.first() != Some(&5) {
        return Ok(None);
    }
    let Some(event) = bytes.get(1) else {
        anyhow::bail!("Noa notification header is truncated");
    };
    let needed = match event {
        18 | 26 | 28 => 3,
        19 => 7,
        29 => 11,
        _ => return Ok(None),
    };
    ensure!(
        bytes.len() >= needed,
        "Noa notification payload is truncated"
    );
    Ok(Some(match event {
        18 => Notification::Input { source: bytes[2] },
        19 => Notification::Io {
            headphone: bytes[2],
            usb: bytes[3],
            aux: bytes[4],
            optical: bytes[5],
            bluetooth: bytes[6],
        },
        26 => Notification::Mode { mode: bytes[2] },
        28 => Notification::Output { state: bytes[2] },
        29 => Notification::Camera {
            locked: bytes[2],
            direction: f32::from_le_bytes(bytes[3..7].try_into()?),
            distance: f32::from_le_bytes(bytes[7..11].try_into()?),
        },
        _ => unreachable!(),
    }))
}

pub fn transport() -> &'static DeviceReadCapability {
    static CAP: OnceLock<DeviceReadCapability> = OnceLock::new();
    CAP.get_or_init(|| {
        serde_json::from_value(serde_json::json!({
            "product_id":1352,"source_class":"rzDevice25Noa","direct_pids":[1352],
            "vendor_id":5426,"claim_interface":4,"report_id":8,"report_bytes":65,
            "transaction_prefix":0,"transaction_modulus":31,"queries":[],
            "polling_codes":{},"charging_codes":{},"max_retry_in":10,"max_retry_out":20,
            "sleep_between_out_ms":0,"sleep_between_out_in_ms":5,"sleep_between_in_ms":5
        }))
        .expect("current Noa source transport")
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Operation {
    ReadIo,
    Bass(u8),
    Output { headset: bool },
    Mode(u8),
    Bluetooth,
}
impl Operation {
    pub fn is_read(self) -> bool {
        matches!(self, Self::ReadIo)
    }
    pub fn validate(self) -> anyhow::Result<()> {
        ensure!(
            !matches!(self, Self::Bass(level) if !(1..=7).contains(&level)),
            "Noa bass level must be 1..7"
        );
        ensure!(
            !matches!(self, Self::Mode(mode) if mode > 3),
            "Noa audio mode must be 0..3"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Acknowledgement {
    pub command: [u8; 3],
    pub transaction: u8,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Report {
    pub requested: Operation,
    pub acknowledgements: Vec<Acknowledgement>,
    pub setter_sent: bool,
    pub headphone_connected: Option<bool>,
    pub previous_input: Option<u8>,
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    data: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Acknowledgement> {
    ensure!(
        cap.report_bytes == 65
            && cap.report_id == 8
            && transaction < 31
            && command[0] <= 54
            && data.len() <= usize::from(command[0]),
        "Noa physical frame differs"
    );
    let mut outgoing = [0u8; 65];
    outgoing[0] = 8;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + data.len()].copy_from_slice(data);
    // Parent checksum XORs body[2..88]. Bytes outside the shorter typed array
    // coerce to zero in JavaScript; body[62] is still zero when it is computed.
    outgoing[63] = outgoing[3..63].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last = "No Noa response".to_owned();
    for _ in 0..cap.max_retry_out {
        validate()?;
        if let Err(error) = device.send_feature_at_collection_max(&outgoing) {
            last = format!("{error:#}");
            continue;
        }
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = [0; 65];
            incoming[0] = 8;
            let count = device
                .get_feature_at_collection_max(&mut incoming)
                .context("Noa GET failed after SET; device state is not confirmed")?;
            validate()?;
            ensure!(count == 65, "Noa response length differs");
            if incoming[2] != transaction || incoming[7..9] != command[1..] {
                last = "Noa transaction/command mismatch".into();
                break;
            }
            match incoming[1] {
                2 => {
                    let size = usize::from(incoming[6]);
                    ensure!(size <= 54, "Noa response payload is truncated");
                    return Ok(Acknowledgement {
                        command,
                        transaction,
                        data: incoming[9..9 + size].to_vec(),
                    });
                }
                0 | 3 => {
                    last = format!("Noa response status {}", incoming[1]);
                    break;
                }
                // The Noa override keeps GET retrying on timeout as well as busy.
                1 | 4 => last = format!("Noa response status {}", incoming[1]),
                status => anyhow::bail!("Noa terminal response status {status}"),
            }
        }
    }
    anyhow::bail!("Noa retry budget exhausted: {last}")
}

pub fn execute(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    operation: Operation,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    operation.validate()?;
    let mut report = Report {
        requested: operation,
        acknowledgements: Vec::new(),
        setter_sent: false,
        headphone_connected: None,
        previous_input: None,
    };
    let mut command = |header, data: &[u8]| exchange(device, cap, header, data, next()?, &validate);
    match operation {
        Operation::ReadIo => {
            let ack = command([5, 8, 139], &[])?;
            ensure!(ack.data.len() >= 3, "Noa audio I/O response is truncated");
            report.headphone_connected = Some(ack.data[0] == 1);
            report.acknowledgements.push(ack);
        }
        Operation::Bluetooth => {
            let read = command([2, 8, 138], &[0, 0])?;
            let previous = *read
                .data
                .get(1)
                .context("Noa input-source response is truncated")?;
            report.previous_input = Some(previous);
            report.acknowledgements.push(read);
            if previous != 3 {
                report.setter_sent = true;
                report.acknowledgements.push(command([2, 8, 10], &[0, 3])?);
            }
        }
        Operation::Bass(level) => {
            report.setter_sent = true;
            report
                .acknowledgements
                .push(command([2, 8, 7], &[1, level])?);
        }
        Operation::Output { headset } => {
            report.setter_sent = true;
            report
                .acknowledgements
                .push(command([2, 8, 17], &[1, u8::from(headset)])?);
        }
        Operation::Mode(mode) => {
            report.setter_sent = true;
            report
                .acknowledgements
                .push(command([2, 8, 16], &[0, mode])?);
        }
    }
    Ok(report)
}

#[cfg(test)]
#[path = "leviathan_tests.rs"]
mod tests;

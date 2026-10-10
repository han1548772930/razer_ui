//! Current 164/241 TaskRunner brightness: profile 1, receiver region 15.
//! Source: receiver-brightness-current-evidence.json. No OS or vendor DLL.
use super::{backend::FeatureTransport, receiver_capabilities::ReceiverCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const GET: [u8; 3] = [3, 15, 132];
const SET: [u8; 3] = [3, 15, 4];
const PROFILE: u8 = 1;
const REGION: u8 = 15;

#[cfg(test)]
#[path = "receiver_brightness_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Brightness {
    pub profile_id: u8,
    pub region_id: u8,
    pub raw_value: u8,
    pub percent: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct Submission {
    pub requested_percent: u8,
    pub previous: Brightness,
    pub acknowledged: Option<Brightness>,
    pub observed: Option<Brightness>,
    /// True only after a transport send returned success.
    pub write_sent: bool,
    /// A failed transport send can still have delivered data to the device.
    pub write_attempted: bool,
    /// Original TaskRunner completed its pre-read/conditional setter chain.
    /// This does not certify a separate hardware readback.
    pub source_completed: bool,
    pub error: Option<String>,
}

#[derive(Default)]
struct WriteProgress {
    attempted: bool,
    sent: bool,
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &ReceiverCapability,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    progress: &mut WriteProgress,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        matches!(cap.product_id, 164 | 241)
            && cap.report_bytes == 91
            && cap.report_id == 0
            && cap.transaction_prefix == 224
            && cap.transaction_modulus == 31
            && transaction & 0xe0 == 224
            && transaction & 0x1f < 31
            && payload.len() <= usize::from(command[0]),
        "Receiver brightness does not match its audited primary Linker transport"
    );
    let mut outgoing = [0u8; 91];
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last = String::from("No receiver brightness response");
    for _ in 0..cap.max_retry_out {
        validate()?;
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
        validate()?;
        progress.attempted = true;
        let sent = device.send_feature(&outgoing);
        if sent.is_ok() {
            progress.sent = true;
        }
        validate()?;
        if let Err(error) = sent {
            return Err(error).context("Receiver brightness send transport failed; source exits its send loop on an exception");
        }
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        // _getUSBTransferInResult resets this once per OUT, then retains it
        // across IN attempts. Its catch does not clear a preceding busy flag.
        let mut resend_out_command = false;
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = [0u8; 91];
            let reply = device.get_feature(&mut incoming);
            validate()?;
            let count = match reply {
                Ok(count) => count,
                Err(error) if resend_out_command => {
                    last = format!("Receiver brightness getter transport failed: {error:#}");
                    break;
                }
                Err(error) => return Err(error).context("Receiver brightness getter transport failed before source requested an OUT retry"),
            };
            // The backend contract retains Report ID, whereas original host
            // strips it. Only bounds required to safely inspect that packet
            // belong here; source slices accept a shorter returned payload.
            ensure!(
                (9..=incoming.len()).contains(&count) && incoming[0] == 0,
                "Receiver brightness transport packet has no complete header or has a different report ID"
            );
            if incoming[2] != transaction || incoming[7..9] != command[1..] {
                last = "Receiver brightness transaction/command differs".into();
                break;
            }
            match incoming[1] {
                1 => {
                    last = "Receiver is busy".into();
                    resend_out_command = true;
                    continue;
                }
                2 => {}
                0 | 3 | 4 => {
                    last = format!("Receiver brightness status {}", incoming[1]);
                    break;
                }
                other => anyhow::bail!(
                    "Receiver brightness is unsupported or returned unknown status {other}"
                ),
            }
            let bytes = usize::from(incoming[6]);
            return Ok(incoming[9..(9 + bytes).min(count)].to_vec());
        }
    }
    anyhow::bail!("Receiver brightness retry failed: {last}")
}

fn parse(data: &[u8]) -> anyhow::Result<Brightness> {
    ensure!(data.len() >= 3, "Receiver brightness reply is truncated");
    Ok(Brightness {
        profile_id: data[0],
        region_id: data[1],
        raw_value: data[2],
        percent: (u16::from(data[2]) * 100).div_ceil(255) as u8,
    })
}

pub fn read(
    device: &dyn FeatureTransport,
    cap: &ReceiverCapability,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Brightness> {
    parse(&exchange(
        device,
        cap,
        GET,
        &[PROFILE, REGION],
        transaction,
        &validate,
        &mut WriteProgress::default(),
    )?)
}

pub fn apply(
    device: &dyn FeatureTransport,
    cap: &ReceiverCapability,
    percent: u8,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Submission> {
    ensure!(
        percent <= 100,
        "Receiver brightness exceeds source UI range 0..100"
    );
    let previous = read(device, cap, next_transaction()?, &validate)?;
    if previous.percent == percent {
        validate()?;
        return Ok(Submission {
            requested_percent: percent,
            observed: Some(previous.clone()),
            previous,
            acknowledged: None,
            write_sent: false,
            write_attempted: false,
            source_completed: true,
            error: None,
        });
    }
    let raw = (u16::from(percent) * 255 / 100) as u8;
    let mut progress = WriteProgress::default();
    let mut submission = Submission {
        requested_percent: percent,
        previous,
        acknowledged: None,
        observed: None,
        write_sent: false,
        write_attempted: false,
        source_completed: false,
        error: None,
    };
    let result = (|| -> anyhow::Result<()> {
        let acknowledged = parse(&exchange(device, cap, SET, &[PROFILE, REGION, raw],
            next_transaction()?, &validate, &mut progress)
            .context("Receiver brightness write may have reached the device; no acknowledgement confirmed")?)?;
        submission.acknowledged = Some(acknowledged);
        // Current TaskRunner awaits setBrightness and then publishes requested
        // brightness. It does not issue a second getter or compare its value.
        submission.source_completed = true;
        Ok(())
    })();
    submission.write_sent = progress.sent;
    submission.write_attempted = progress.attempted;
    if let Err(error) = result {
        submission.error = Some(format!("{error:#}"));
    }
    Ok(submission)
}

//! Current 164/241 TaskRunner brightness: profile 1, receiver region 15.
//! The retained Feature transport owns platform I/O and actual acknowledgements.
use super::{backend::FeatureTransport, receiver_capabilities::ReceiverCapability};
use crate::receiver_feature_channel::{WriteProgress, exchange};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

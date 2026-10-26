//! Current controller Protocol25 Feature channel, shared by the source trigger
//! and local controller-lighting wrappers. The owner supplies cancelable waits
//! and retains the observed collection for the entire ordered operation.
use crate::backend::FeatureTransport;
use anyhow::{bail, ensure};
use std::time::Duration;

pub(crate) fn report(
    selector: u8,
    transaction: u8,
    header: [u8; 3],
    data: &[u8],
) -> anyhow::Result<[u8; 91]> {
    ensure!(
        header[0] <= 80 && data.len() <= 80,
        "Controller request payload exceeds Protocol25"
    );
    let mut report = [0; 91];
    report[0] = selector;
    report[2] = transaction;
    report[6..9].copy_from_slice(&header);
    report[9..9 + data.len()].copy_from_slice(data);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    Ok(report)
}

pub(crate) fn exchange<T: FeatureTransport + ?Sized>(
    transport: &T,
    request: &[u8; 91],
    delay: Duration,
    wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    exchange_with_in_limit(transport, request, delay, wait, 10)
}

pub(crate) fn exchange_with_in_limit<T: FeatureTransport + ?Sized>(
    transport: &T,
    request: &[u8; 91],
    delay: Duration,
    wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    max_retry_in: u8,
) -> anyhow::Result<Vec<u8>> {
    exchange_with_timing(transport, request, delay, delay, delay, wait, max_retry_in)
}

/// Profile-name reads temporarily extend only OUT/IN and subsequent IN waits.
pub(crate) fn exchange_with_timing<T: FeatureTransport + ?Sized>(
    transport: &T,
    request: &[u8; 91],
    out_delay: Duration,
    out_in_delay: Duration,
    in_delay: Duration,
    wait: &mut impl FnMut(Duration) -> anyhow::Result<()>,
    max_retry_in: u8,
) -> anyhow::Result<Vec<u8>> {
    ensure!(max_retry_in > 0, "Controller reply retry count is zero");
    let mut error = String::from("No controller Feature reply");
    for _ in 0..20 {
        wait(out_delay)?;
        if let Err(e) = transport.send_feature_at_collection_max(request) {
            error = e.to_string();
            continue;
        }
        wait(out_in_delay)?;
        for retry in 0..max_retry_in {
            if retry != 0 {
                wait(in_delay)?;
            }
            let mut response = [0; 91];
            response[0] = request[0];
            let count = match transport.get_feature_at_collection_max(&mut response) {
                Ok(count) => count,
                Err(e) => {
                    error = e.to_string();
                    break;
                }
            };
            ensure!(
                count == response.len(),
                "Truncated controller Feature report"
            );
            // UsbRzDeviceAction removes the returned Report ID before the
            // current middleware checks transaction/class/command.
            if response[2] != request[2] || response[7..9] != request[7..9] {
                error = "Stale controller response".into();
                break;
            }
            match response[1] {
                0 | 3 | 4 => {
                    error = format!("Controller retry status {}", response[1]);
                    break;
                }
                1 => error = "Controller busy".into(),
                2 => {
                    let size = usize::from(response[6]);
                    ensure!(size <= 80, "Invalid controller response payload length");
                    return Ok(response[9..9 + size].to_vec());
                }
                status => bail!("Controller command rejected with status {status}"),
            }
        }
    }
    bail!("Controller Feature retries exhausted: {error}")
}

//! Source-derived 90-byte Razer packet exchange plus the HID report ID.
//! Product-specific callers must gate commands and decode the returned payload.
//! The caller owns the observed collection, transaction counter and session lock.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use std::time::Duration;

pub(crate) fn exchange(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    exchange_inner(
        device,
        cap,
        command,
        payload,
        transaction,
        None,
        validate,
        &mut || {},
    )
}

/// Source `sendCommand` may keep the command header's maximum payload while
/// overriding packet byte 5 (HID report byte 6) with the actual data size. Profile-name
/// blocks use this for 69-byte and 63-byte writes; the ordinary executor keeps
/// its historical report-byte behavior through [`exchange`].
pub(crate) fn exchange_with_report_data_size(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    report_data_size: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    exchange_with_report_data_size_observing_send(
        device,
        cap,
        command,
        payload,
        transaction,
        report_data_size,
        validate,
        &mut || {},
    )
}

/// Observe an actual call to the transport, after layout/transaction checks and
/// the cancellation guard. A rejected request must not be reported as a write.
pub(crate) fn exchange_with_report_data_size_observing_send(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    report_data_size: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    observe_send: &mut impl FnMut(),
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        (1..=80).contains(&report_data_size) && usize::from(report_data_size) >= payload.len(),
        "Feature report data size must be 1..80 and contain the outgoing payload"
    );
    exchange_inner(
        device,
        cap,
        command,
        payload,
        transaction,
        Some(report_data_size),
        validate,
        observe_send,
    )
}

fn exchange_inner(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    report_data_size: Option<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
    observe_send: &mut impl FnMut(),
) -> anyhow::Result<Vec<u8>> {
    let (bound, transaction) = cap.for_exchange(device, transaction)?;
    let cap = bound.as_ref();
    let get_report_id = cap.get_report_id_for_transport(device)?;
    ensure!(
        cap.report_bytes == 91
            && command[0] <= 80
            && payload.len() <= usize::from(command[0])
            && cap.max_retry_out > 0
            && cap.max_retry_in > 0,
        "Source Feature packet layout or retry limits do not match"
    );
    ensure!(
        (cap.transaction_prefix & 0x1F == 0 || cap.transaction_prefix == 0xF0)
            && (1..=32).contains(&cap.transaction_modulus)
            && (0..cap.transaction_modulus)
                .any(|counter| cap.transaction_prefix | counter == transaction),
        "Feature transaction is outside the current source namespace"
    );
    let mut outgoing = [0u8; 91];
    outgoing[0] = cap.report_id;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    if let Some(size) = report_data_size {
        outgoing[6] = size;
    }
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last_error = "No Feature response received".to_owned();
    for _ in 0..cap.max_retry_out {
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
        validate()?;
        observe_send();
        let sent = device.send_feature_at_collection_max(&outgoing);
        validate()?;
        if let Err(error) = sent {
            last_error = format!("{error:#}");
            continue;
        }
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = [0u8; 91];
            incoming[0] = get_report_id;
            let received = device.get_feature_at_collection_max(&mut incoming);
            validate()?;
            let count = match received {
                Ok(count) => count,
                Err(error) => {
                    last_error = format!("{error:#}");
                    break;
                }
            };
            ensure!(count == incoming.len(), "Feature response is truncated");
            // The original host slices off the returned prefix without checking
            // it against the request's interface/report selector.
            let packet = &incoming[1..];
            if packet[1] != transaction || packet[6..8] != command[1..] {
                last_error = "Feature response transaction or command does not match".into();
                break;
            }
            match packet[0] {
                1 => {
                    last_error = "Device is busy".into();
                    continue;
                }
                2 => {}
                0 | 3 | 4 => {
                    last_error = format!("Feature response status {}", packet[0]);
                    break;
                }
                state => anyhow::bail!("Feature command is unsupported or returned status {state}"),
            }
            let length = usize::from(packet[5]);
            ensure!(
                length <= 80,
                "Feature response data exceeds the packet buffer"
            );
            return Ok(packet[8..8 + length].to_vec());
        }
    }
    Err(anyhow::anyhow!(last_error)).context("Source Feature command retries exhausted")
}

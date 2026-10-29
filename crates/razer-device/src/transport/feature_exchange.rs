//! Source-derived 90-byte Razer packet exchange plus the HID report ID.
//! Product-specific callers must gate commands and decode the returned payload.
//! The caller owns the observed collection, transaction counter and session lock.
use crate::{
    backend::{ExchangeLease, ExchangeOptions, FeatureTransport},
    device_reads::DeviceReadCapability,
};
use anyhow::{Context as _, ensure};
use std::time::Duration;

/// These exact factories have current source evidence for the navigator lock
/// family. Other factories keep collection admission without claiming its key.
pub(crate) fn with_protocol25_scope(
    options: ExchangeOptions,
    source_class: &str,
) -> ExchangeOptions {
    use crate::backend::{Protocol25Topology, SourceCommandScope};
    let topology = match source_class {
        "rzDevice25" => Protocol25Topology::Normal,
        "rzDevice25DualLinkMouse" | "rzDevice25DualLinkKeyboard" => Protocol25Topology::DualLink,
        _ => return options,
    };
    options.with_source_scope(SourceCommandScope::Protocol25 { topology })
}

/// Keep one admitted collection operation across all of its protocol retries.
/// The source releases its HID mutex in finally, including protocol errors.
/// Canceled or stale owners are rejected by the retained lease's finish path.
pub(crate) fn with_exchange<T: FeatureTransport + ?Sized, R>(
    transport: &T,
    options: ExchangeOptions,
    operation: impl FnOnce(&dyn FeatureTransport, &dyn ExchangeLease) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let lease = transport.begin_exchange(options)?;
    let result = operation(lease.transport(), lease.as_ref());
    crate::backend::finish_exchange(lease, result)
}

pub(crate) fn finish_if_current<R>(
    lease: Box<dyn ExchangeLease + '_>,
    result: anyhow::Result<R>,
    current: anyhow::Result<()>,
) -> anyhow::Result<R> {
    match current {
        Ok(()) => crate::backend::finish_exchange(lease, result),
        Err(validation) => {
            drop(lease);
            Err(crate::backend::finalization_failure(match result {
                Ok(_) => validation,
                Err(error) => error.context(format!(
                    "HID exchange owner is no longer current: {validation:#}"
                )),
            }))
        }
    }
}

pub(crate) fn with_validated_exchange<T: FeatureTransport + ?Sized, R>(
    transport: &T,
    options: ExchangeOptions,
    validate: &dyn Fn() -> anyhow::Result<()>,
    operation: impl FnOnce(&dyn FeatureTransport, &dyn ExchangeLease) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    validate()?;
    let lease = transport.begin_exchange_validated(options, std::rc::Rc::new(validate))?;
    let result = operation(lease.transport(), lease.as_ref());
    finish_if_current(lease, result, validate())
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CompletedFeatureExchange {
    command: [u8; 3],
    transaction: u8,
    data: Vec<u8>,
}

impl CompletedFeatureExchange {
    pub fn from_error(error: &anyhow::Error) -> Option<&Self> {
        error.downcast_ref()
    }

    pub fn command(&self) -> [u8; 3] {
        self.command
    }

    pub fn transaction(&self) -> u8 {
        self.transaction
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl std::fmt::Display for CompletedFeatureExchange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Feature response decoded for command {:?}, transaction {}; exchange finalization failed",
            self.command, self.transaction
        )
    }
}

/// Retain a protocol-validated response as diagnostic evidence when the final
/// owner check or release fails. Such evidence does not authorize publishing
/// state to the current connection or continuing the command sequence.
pub(crate) fn acknowledged_reply(result: &anyhow::Result<Vec<u8>>) -> Option<&[u8]> {
    match result {
        Ok(data) => Some(data),
        Err(error) => acknowledged_error(error),
    }
}

pub(crate) fn acknowledged_error(error: &anyhow::Error) -> Option<&[u8]> {
    error
        .downcast_ref::<CompletedFeatureExchange>()
        .map(|completed| completed.data.as_slice())
}

/// The operation must return only after its transport guard, transaction,
/// status and payload-length checks have accepted the response.
pub(crate) fn with_validated_feature_reply<T: FeatureTransport + ?Sized>(
    transport: &T,
    options: ExchangeOptions,
    command: [u8; 3],
    transaction: u8,
    validate: &dyn Fn() -> anyhow::Result<()>,
    operation: impl FnOnce(&dyn FeatureTransport, &dyn ExchangeLease) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<Vec<u8>> {
    validate()?;
    let lease = transport.begin_exchange_validated(options, std::rc::Rc::new(validate))?;
    let result = operation(lease.transport(), lease.as_ref());
    let current = validate();
    finish_feature_reply(lease, result, current, command, transaction)
}

pub(crate) fn with_feature_reply<T: FeatureTransport + ?Sized>(
    transport: &T,
    options: ExchangeOptions,
    command: [u8; 3],
    transaction: u8,
    operation: impl FnOnce(&dyn FeatureTransport, &dyn ExchangeLease) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<Vec<u8>> {
    let lease = transport.begin_exchange(options)?;
    let result = operation(lease.transport(), lease.as_ref());
    finish_feature_reply(lease, result, Ok(()), command, transaction)
}

fn finish_feature_reply(
    lease: Box<dyn ExchangeLease + '_>,
    result: anyhow::Result<Vec<u8>>,
    current: anyhow::Result<()>,
    command: [u8; 3],
    transaction: u8,
) -> anyhow::Result<Vec<u8>> {
    match result {
        Ok(data) => match finish_if_current(lease, Ok(()), current) {
            Ok(()) => Ok(data),
            Err(error) => Err(error.context(CompletedFeatureExchange {
                command,
                transaction,
                data,
            })),
        },
        Err(error) => finish_if_current(lease, Err(error), current),
    }
}

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
/// blocks use this for 69-byte and 63-byte writes. [`exchange`] instead copies
/// the command header's data size into report byte 6.
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
    with_validated_feature_reply(
        device,
        with_protocol25_scope(
            ExchangeOptions::mutex_out(Duration::from_millis(cap.sleep_between_out_ms)),
            &cap.source_class,
        ),
        command,
        transaction,
        validate,
        |device, lease| {
            let mut last_error = "No Feature response received".to_owned();
            for out_attempt in 0..cap.max_retry_out {
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
                        last_error =
                            "Feature response transaction or command does not match".into();
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
                        state => anyhow::bail!(
                            "Feature command is unsupported or returned status {state}"
                        ),
                    }
                    let length = usize::from(packet[5]);
                    ensure!(
                        length <= 80,
                        "Feature response data exceeds the packet buffer"
                    );
                    return Ok(packet[8..8 + length].to_vec());
                }
                if out_attempt + 1 < cap.max_retry_out {
                    lease.retry_release()?;
                }
            }
            Err(anyhow::anyhow!(last_error)).context("Source Feature command retries exhausted")
        },
    )
}

#[cfg(test)]
#[path = "feature_exchange_tests.rs"]
pub(crate) mod tests;

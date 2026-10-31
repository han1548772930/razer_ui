//! Current controller Protocol25 Feature channel, shared by the source trigger
//! and local controller-lighting wrappers. The owner supplies cancelable waits
//! and retains the observed collection for the entire ordered operation.
use crate::backend::{
    FeatureTransport, complete_native_io, complete_native_read, native_read_bytes,
    require_complete_send,
};
use anyhow::{Context as _, bail, ensure};
use std::time::Duration;

enum Reply {
    Acknowledged(Vec<u8>),
    Busy,
    Resend(&'static str),
    Rejected(u8),
}

fn decode_reply(response: &[u8], request: &[u8; 91]) -> Reply {
    // UsbRzDeviceAction removes the returned Report ID without matching it.
    // rzDevice25 checks the actual header and slices only available payload.
    if response.len() < 9 {
        return Reply::Resend("Truncated controller response header");
    }
    if response[2] != request[2] || response[7..9] != request[7..9] {
        return Reply::Resend("Stale controller response");
    }
    match response[1] {
        0 => Reply::Resend("Controller reported a new command"),
        1 => Reply::Busy,
        2 => {
            let end = (9 + usize::from(response[6])).min(response.len());
            Reply::Acknowledged(response[9..end].to_vec())
        }
        3 => Reply::Resend("Controller command failed"),
        4 => Reply::Resend("Controller command timed out"),
        status => Reply::Rejected(status),
    }
}

/// Source best-effort wrappers must preserve admission, native completion and
/// release failures before converting ordinary command errors into diagnostics.
pub(crate) fn terminal_io_failure(error: &anyhow::Error) -> bool {
    crate::backend::exchange_interrupted(error)
}

fn retain_failed_reply(error: anyhow::Error, request: &[u8; 91]) -> anyhow::Error {
    let data = native_read_bytes(&error)
        .filter(|bytes| bytes.len() <= request.len())
        .and_then(|bytes| match decode_reply(bytes, request) {
            Reply::Acknowledged(data) => Some(data),
            _ => None,
        });
    match data {
        Some(data) => crate::feature_exchange::CompletedFeatureExchange::retain(
            error,
            [request[6], request[7], request[8]],
            request[2],
            data,
        ),
        None => error,
    }
}

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
    let mut guarded_wait = |delay| {
        wait(delay).map_err(|error| {
            crate::backend::interrupt_exchange(error, crate::backend::ExchangeInterruption::Wait)
        })
    };
    crate::feature_exchange::with_feature_reply(
        transport,
        crate::feature_exchange::with_protocol25_scope(
            crate::backend::ExchangeOptions::mutex_out(out_delay),
            "rzDevice25",
        ),
        [request[6], request[7], request[8]],
        request[2],
        |transport, lease| {
            let mut error = anyhow::anyhow!("No controller Feature reply");
            for out_attempt in 0..20 {
                guarded_wait(Duration::ZERO)?;
                let sent = transport.send_feature_at_collection_max_counted(request);
                if let Err(validation) = guarded_wait(Duration::ZERO) {
                    return Err(complete_native_io(sent, validation));
                }
                match sent {
                    Ok(count) if count == request.len() => {}
                    Ok(count) => {
                        let incomplete = require_complete_send(count, request.len())
                            .expect_err("the normal return count differs from the request");
                        error = incomplete;
                        continue;
                    }
                    Err(failure) => {
                        if terminal_io_failure(&failure) {
                            return Err(failure);
                        }
                        // The current host/IPC helpers swallow ordinary native
                        // errors as undefined. sendCommand retries without IN.
                        error = failure;
                        continue;
                    }
                }
                guarded_wait(out_in_delay)?;
                for retry in 0..max_retry_in {
                    if retry != 0 {
                        guarded_wait(in_delay)?;
                    }
                    guarded_wait(Duration::ZERO)?;
                    let mut response = [0; 91];
                    response[0] = request[0];
                    let received = transport.get_feature_at_collection_max(&mut response);
                    if let Err(validation) = guarded_wait(Duration::ZERO) {
                        return Err(retain_failed_reply(
                            complete_native_read(received, &response, validation),
                            request,
                        ));
                    }
                    let count = match received {
                        Ok(count) if count <= response.len() => count,
                        Ok(count) => bail!("Impossible controller read byte count {count}"),
                        Err(failure) => {
                            if terminal_io_failure(&failure) {
                                return Err(retain_failed_reply(failure, request));
                            }
                            // An absent host result cannot acknowledge data.
                            // Its source zero-header branch requests a resend.
                            error = failure;
                            break;
                        }
                    };
                    match decode_reply(&response[..count], request) {
                        Reply::Acknowledged(data) => return Ok(data),
                        Reply::Busy => error = anyhow::anyhow!("Controller busy"),
                        Reply::Resend(reason) => {
                            error = anyhow::anyhow!(reason);
                            break;
                        }
                        Reply::Rejected(status) => {
                            bail!("Controller command rejected with status {status}")
                        }
                    }
                }
                if out_attempt + 1 < 20 {
                    guarded_wait(Duration::ZERO)?;
                    lease.retry_release()?;
                }
            }
            Err(error).context("Controller Feature retries exhausted")
        },
    )
}

#[cfg(test)]
#[path = "controller_feature_tests.rs"]
mod tests;

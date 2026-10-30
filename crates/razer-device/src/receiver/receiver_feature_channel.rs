//! Primary Linker Feature exchange. Brightness and lighting retain the same
//! source transaction/status/retry semantics and observed collection boundary.
use crate::{
    backend::{ExchangeOptions, FeatureTransport},
    receiver_capabilities::ReceiverCapability,
};
use anyhow::{Context as _, ensure};
use std::time::Duration;

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct WriteProgress {
    pub attempted: bool,
    pub sent: bool,
    pub acknowledged: bool,
    pub acknowledged_reply: Option<Vec<u8>>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ReceiverExchangeFailure {
    pub command: [u8; 3],
    pub transaction: u8,
    pub progress: WriteProgress,
}

impl std::fmt::Display for ReceiverExchangeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Receiver exchange failed for {:?}, transaction {}",
            self.command, self.transaction
        )
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ReceiverFrameSubmissionFailure {
    pub reports_submitted: u32,
}

impl std::fmt::Display for ReceiverFrameSubmissionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Receiver frame exchange failed after submitting {} reports",
            self.reports_submitted
        )
    }
}

pub(crate) struct Channel {
    pub source_scope: Option<crate::backend::SourceCommandScope>,
    pub report_id: u8,
    pub get_report_id: u8,
    pub transaction_prefix: u8,
    pub transaction_modulus: u8,
    pub max_retry_in: u8,
    pub max_retry_out: u8,
    pub sleep_between_out_ms: u64,
    pub sleep_between_out_in_ms: u64,
    pub sleep_between_in_ms: u64,
    pub ignore_set_device_mode_timeout_error: bool,
}

pub(crate) fn exchange(
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
            && cap.report_id == cap.claim_interface
            && cap.transaction_prefix == 224
            && cap.transaction_modulus == 31
            && transaction & 0xe0 == 224
            && transaction & 0x1f < 31,
        "Receiver brightness does not match its audited primary Linker transport"
    );
    exchange_channel(
        device,
        &Channel {
            source_scope: crate::feature_exchange::with_protocol25_scope(
                ExchangeOptions::plain(),
                &cap.source_class,
            )
            .source_scope,
            report_id: cap.report_id,
            get_report_id: cap.get_report_id(),
            transaction_prefix: cap.transaction_prefix,
            transaction_modulus: cap.transaction_modulus,
            max_retry_in: cap.max_retry_in,
            max_retry_out: cap.max_retry_out,
            sleep_between_out_ms: cap.sleep_between_out_ms,
            sleep_between_out_in_ms: cap.sleep_between_out_in_ms,
            sleep_between_in_ms: cap.sleep_between_in_ms,
            ignore_set_device_mode_timeout_error: false,
        },
        command,
        payload,
        transaction,
        validate,
        progress,
    )
}

pub(crate) fn exchange_channel(
    device: &dyn FeatureTransport,
    cap: &Channel,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    progress: &mut WriteProgress,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        command[0] <= 80
            && payload.len() <= usize::from(command[0])
            && cap.max_retry_out > 0
            && cap.max_retry_in > 0
            && (0..cap.transaction_modulus)
                .any(|counter| cap.transaction_prefix | counter == transaction),
        "Feature command differs from its source transport namespace"
    );
    let mut outgoing = [0u8; 91];
    outgoing[0] = cap.report_id;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut options = ExchangeOptions::mutex_out(Duration::from_millis(cap.sleep_between_out_ms));
    options.source_scope = cap.source_scope;
    let result = crate::feature_exchange::with_validated_exchange(
        device,
        options,
        validate,
        |device, lease| {
            let mut last = String::from("No receiver brightness response");
            for out_attempt in 0..cap.max_retry_out {
                validate()?;
                let sent = device.send_feature_at_collection_max_counted(&outgoing);
                progress.attempted |= crate::backend::native_io_attempted(&sent);
                progress.sent |= match &sent {
                    Ok(count) => *count == outgoing.len(),
                    Err(error) => matches!(
                        crate::backend::native_io_outcome(error),
                        Some(crate::backend::NativeIoOutcome::Returned { byte_count })
                            if byte_count == outgoing.len()
                    ),
                };
                if let Err(validation) = validate() {
                    return Err(crate::backend::complete_native_io(sent, validation));
                }
                if let Err(error) = sent
                    .and_then(|count| crate::backend::require_complete_send(count, outgoing.len()))
                {
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
                    incoming[0] = cap.get_report_id;
                    let reply = device.get_feature_at_collection_max(&mut incoming);
                    if let Err(validation) = validate() {
                        return Err(crate::backend::complete_native_io(reply, validation));
                    }
                    let count = match reply {
                Ok(count) => count,
                Err(error) if crate::backend::exchange_finalization_failed(&error) => return Err(error),
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
                        (9..=incoming.len()).contains(&count),
                        "Receiver brightness transport packet has no complete header"
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
                        2 => {
                            progress.acknowledged = true;
                        }
                        4 if cap.ignore_set_device_mode_timeout_error && command == [2, 0, 4] => {}
                        0 | 3 | 4 => {
                            last = format!("Receiver brightness status {}", incoming[1]);
                            break;
                        }
                        other => anyhow::bail!(
                            "Receiver brightness is unsupported or returned unknown status {other}"
                        ),
                    }
                    let bytes = usize::from(incoming[6]);
                    let data = incoming[9..(9 + bytes).min(count)].to_vec();
                    if incoming[1] == 2 {
                        progress.acknowledged_reply = Some(data.clone());
                    }
                    return Ok(data);
                }
                if out_attempt + 1 < cap.max_retry_out {
                    lease.retry_release()?;
                }
            }
            anyhow::bail!("Receiver brightness retry failed: {last}")
        },
    );
    result.map_err(|error| {
        error.context(ReceiverExchangeFailure {
            command,
            transaction,
            progress: progress.clone(),
        })
    })
}

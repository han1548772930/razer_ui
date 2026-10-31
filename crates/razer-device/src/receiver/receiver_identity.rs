//! Current rzDevice25 getSerialNumber: command [22,0,130], printable ASCII only.
use super::{
    backend::{ExchangeOptions, FeatureTransport},
    receiver_capabilities::ReceiverCapability,
};
use anyhow::{bail, ensure};
use std::time::Duration;
pub fn read_serial(
    device: &dyn FeatureTransport,
    cap: &ReceiverCapability,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<String> {
    let validate = &|| {
        validate().map_err(|error| {
            crate::backend::interrupt_exchange(
                error,
                crate::backend::ExchangeInterruption::Validation,
            )
        })
    };
    ensure!(
        matches!(cap.product_id, 164 | 241)
            && cap.report_bytes == 91
            && cap.report_id == cap.claim_interface,
        "Serial query has no audited receiver capability"
    );
    let mut report = vec![0_u8; 91];
    report[0] = cap.report_id;
    report[2] = transaction;
    report[6..9].copy_from_slice(&[22, 0, 130]);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    crate::feature_exchange::with_validated_exchange(
        device,
        crate::feature_exchange::with_protocol25_scope(
            ExchangeOptions::mutex_out(Duration::from_millis(cap.sleep_between_out_ms)),
            &cap.source_class,
        ),
        &validate,
        |device, lease| {
            let mut last = String::from("No serial response");
            for out_attempt in 0..cap.max_retry_out {
                validate()?;
                let sent = device.send_feature_at_collection_max_counted(&report);
                if let Err(validation) = validate() {
                    return Err(crate::backend::complete_native_io(sent, validation));
                }
                sent.and_then(|count| crate::backend::require_complete_send(count, report.len()))?;
                std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
                let mut resend_out_command = false;
                for attempt in 0..cap.max_retry_in {
                    if attempt > 0 {
                        std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
                    }
                    validate()?;
                    let mut input = vec![0_u8; 91];
                    input[0] = cap.get_report_id();
                    let received = device.get_feature_at_collection_max(&mut input);
                    let received = match validate() {
                        Ok(()) => received,
                        Err(validation) => Err(crate::backend::complete_native_read(
                            received, &input, validation,
                        )),
                    };
                    let count = match received {
                        Ok(count) => count,
                        Err(error) if crate::backend::exchange_interrupted(&error) => {
                            return Err(crate::receiver_feature_channel::retain_failed_reply(
                                error,
                                [22, 0, 130],
                                transaction,
                            ));
                        }
                        Err(error) if resend_out_command => {
                            last = error.to_string();
                            break;
                        }
                        Err(error) => return Err(error),
                    };
                    ensure!(
                        (9..=input.len()).contains(&count),
                        "Serial Feature report has no complete header"
                    );
                    if input[2] != transaction || input[7..9] != [0, 130] {
                        last = "Serial response transaction/command differs".into();
                        break;
                    }
                    match input[1] {
                        1 => {
                            resend_out_command = true;
                            continue;
                        }
                        2 => {}
                        0 | 3 | 4 => {
                            last = format!("Serial response status {}", input[1]);
                            break;
                        }
                        other => {
                            bail!(
                                "Receiver serial query is unsupported or returned unknown status {other}"
                            )
                        }
                    }
                    let len = usize::from(input[6]);
                    let serial: String = input[9..(9 + len).min(count)]
                        .iter()
                        .copied()
                        .filter(|byte| (32..127).contains(byte))
                        .map(char::from)
                        .collect();
                    return Ok(serial);
                }
                if out_attempt + 1 < cap.max_retry_out {
                    lease.retry_release()?;
                }
            }
            bail!("Receiver serial query failed: {last}")
        },
    )
}

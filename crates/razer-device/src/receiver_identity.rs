//! Current rzDevice25 getSerialNumber: command [22,0,130], printable ASCII only.
use super::{backend::FeatureTransport, receiver_capabilities::ReceiverCapability};
use anyhow::{bail, ensure};
use std::time::Duration;
pub fn read_serial(
    device: &dyn FeatureTransport,
    cap: &ReceiverCapability,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<String> {
    ensure!(
        matches!(cap.product_id, 164 | 241) && cap.report_bytes == 91 && cap.report_id == 0,
        "Serial query has no audited receiver capability"
    );
    let mut report = vec![0_u8; 91];
    report[2] = transaction;
    report[6..9].copy_from_slice(&[22, 0, 130]);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last = String::from("No serial response");
    for _ in 0..cap.max_retry_out {
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
        validate()?;
        if let Err(error) = device.send_feature(&report) {
            last = error.to_string();
            continue;
        }
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            validate()?;
            let mut input = vec![0_u8; 91];
            let count = match device.get_feature(&mut input) {
                Ok(count) => count,
                Err(error) => {
                    last = error.to_string();
                    break;
                }
            };
            validate()?;
            ensure!(
                count == 91 && input[0] == 0,
                "Serial Feature report is truncated or has a different ReportID"
            );
            if input[2] != transaction || input[7..9] != [0, 130] {
                last = "Serial response transaction/command differs".into();
                break;
            }
            match input[1] {
                1 => continue,
                2 => {}
                5 => bail!("Receiver does not support serial query"),
                other => {
                    last = format!("Serial response status {other}");
                    break;
                }
            }
            let len = usize::from(input[6]);
            ensure!(
                (1..=80).contains(&len),
                "Serial response data length is invalid"
            );
            let serial: String = input[9..9 + len]
                .iter()
                .copied()
                .filter(|byte| (32..127).contains(byte))
                .map(char::from)
                .collect();
            ensure!(
                !serial.is_empty(),
                "Serial reply has no printable source serial; local storage cannot bind this owner"
            );
            return Ok(serial);
        }
    }
    bail!("Receiver serial query failed: {last}")
}

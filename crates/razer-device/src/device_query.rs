//! Shared Razer query execution over an OS-neutral Feature transport.
//! Timing/commands/replies come from current generated source capabilities.
//! Target identity, locks and transaction allocation remain caller responsibilities.
use super::{
    backend::FeatureTransport,
    device_reads::{self, DeviceReadCapability, DeviceReadValue, ReadCommand, ReadReply},
};
use anyhow::{Context as _, ensure};
use std::time::Duration;

pub fn read_device(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: &ReadCommand,
    transaction: u8,
    deadline: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DeviceReadValue> {
    let outgoing = device_reads::query_report(cap, command, transaction)?;
    let mut last_error = "没有收到设备响应".to_string();
    let mut reading = None;
    'send: for _ in 0..cap.max_retry_out {
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
        deadline()?;
        match device.send_feature(&outgoing) {
            Ok(()) => {}
            Err(error) => {
                last_error = format!("{error:#}");
                continue;
            }
        }
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            deadline()?;
            let mut report = vec![0; cap.report_bytes];
            report[0] = cap.report_id;
            let count = match device.get_feature(&mut report) {
                Ok(count) => count,
                Err(error) => {
                    // The current host turns a failed getFeatureReport into
                    // an empty response; middleware retries with a new OUT.
                    last_error = format!("{error:#}");
                    break;
                }
            };
            deadline()?;
            ensure!(count == cap.report_bytes, "设备查询返回长度不匹配：{count}");
            match device_reads::decode_report(&report, cap, command, transaction)? {
                ReadReply::Busy => last_error = "设备忙".into(),
                ReadReply::Retry(reason) => {
                    last_error = reason;
                    break;
                }
                ReadReply::Complete(value) => {
                    reading = Some(value);
                    break 'send;
                }
            }
        }
    }
    reading.with_context(|| format!("设备查询重试失败：{last_error}"))
}

pub fn read_receiver(
    device: &dyn FeatureTransport,
    selected: &super::receiver_capabilities::ReceiverCapability,
    transaction: u8,
    deadline: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<(u16, u8)>> {
    use super::receiver_protocol::{self as protocol, Reply};
    use anyhow::bail;
    let outgoing = protocol::query_report(selected, transaction)?;
    let mut last_error = "尚未取得连接状态".to_owned();
    // Exact factory timing/retry parameters are source-derived, including the
    // factory's multiplier. Keep one transaction across retries, as source does.
    for _ in 0..selected.max_retry_out {
        std::thread::sleep(Duration::from_millis(selected.sleep_between_out_ms));
        deadline()?;
        let sent = device.send_feature(&outgoing);
        deadline()?;
        match sent {
            Ok(()) => {}
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        }
        std::thread::sleep(Duration::from_millis(selected.sleep_between_out_in_ms));
        for attempt in 0..selected.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(selected.sleep_between_in_ms));
            }
            deadline()?;
            let mut report = vec![0u8; selected.report_bytes];
            report[0] = selected.report_id;
            let received = device.get_feature(&mut report);
            deadline()?;
            let received = match received {
                Ok(count) => count,
                Err(error) => {
                    last_error = error.to_string();
                    break;
                }
            };
            ensure!(
                received == report.len(),
                "原生 HID 返回 {received} 字节，期望 {}",
                report.len()
            );
            match protocol::decode_report(&report, selected, transaction)? {
                Reply::Busy => {
                    last_error = "设备忙，查询未完成".into();
                }
                Reply::Retry(reason) => {
                    last_error = reason.into();
                    break;
                }
                Reply::Complete(devices) => {
                    return Ok(devices);
                }
            }
        }
    }
    bail!("接收器查询重试已耗尽：{last_error}")
}

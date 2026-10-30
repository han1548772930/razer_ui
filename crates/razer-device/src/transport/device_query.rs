//! Shared Razer query execution over an OS-neutral Feature transport.
//! Timing/commands/replies come from current generated source capabilities.
//! Target identity, locks and transaction allocation remain caller responsibilities.
use super::{
    backend::{ExchangeOptions, FeatureTransport},
    device_reads::{self, DeviceReadCapability, DeviceReadValue, ReadCommand, ReadReply},
};
use anyhow::{Context as _, ensure};
use std::time::Duration;

#[cfg(test)]
#[path = "device_query_tests.rs"]
mod tests;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CompletedDeviceRead {
    value: DeviceReadValue,
}

impl CompletedDeviceRead {
    pub fn from_error(error: &anyhow::Error) -> Option<&Self> {
        error.downcast_ref()
    }

    pub fn value(&self) -> &DeviceReadValue {
        &self.value
    }
}

impl std::fmt::Display for CompletedDeviceRead {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Device reply decoded before exchange finalization failed")
    }
}

fn retain_failed_device_read(
    error: anyhow::Error,
    cap: &DeviceReadCapability,
    command: &ReadCommand,
    transaction: u8,
) -> anyhow::Error {
    let value =
        crate::backend::native_read_bytes(&error).and_then(
            |bytes| match device_reads::decode_report(bytes, cap, command, transaction) {
                Ok(ReadReply::Complete(value)) => Some(value),
                _ => None,
            },
        );
    match value {
        Some(value) => error.context(CompletedDeviceRead { value }),
        None => error,
    }
}

/// A decoded list retained after ownership or release failure is diagnostic;
/// it does not establish the current receiver's connected device state.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CompletedReceiverRead {
    devices: Vec<(u16, u8)>,
}

impl CompletedReceiverRead {
    pub fn from_error(error: &anyhow::Error) -> Option<&Self> {
        error.downcast_ref()
    }

    pub fn devices(&self) -> &[(u16, u8)] {
        &self.devices
    }
}

impl std::fmt::Display for CompletedReceiverRead {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Receiver device list decoded before exchange finalization failed")
    }
}

fn retain_failed_receiver_read(
    error: anyhow::Error,
    selected: &super::receiver_capabilities::ReceiverCapability,
    transaction: u8,
) -> anyhow::Error {
    let devices = crate::backend::native_read_bytes(&error).and_then(|bytes| {
        match super::receiver_protocol::decode_report(bytes, selected, transaction) {
            Ok(super::receiver_protocol::Reply::Complete(devices)) => Some(devices),
            _ => None,
        }
    });
    match devices {
        Some(devices) => error.context(CompletedReceiverRead { devices }),
        None => error,
    }
}

/// A late failure preserves the parsed response for diagnostics. It remains an
/// error and must not be published as the current connection's observed state.
pub(crate) fn completed_reading(
    result: &anyhow::Result<DeviceReadValue>,
) -> Option<&DeviceReadValue> {
    match result {
        Ok(value) => Some(value),
        Err(error) => error
            .downcast_ref::<CompletedDeviceRead>()
            .map(|completed| &completed.value),
    }
}

pub fn read_device(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: &ReadCommand,
    transaction: u8,
    deadline: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DeviceReadValue> {
    let (bound, transaction) = cap.for_exchange(device, transaction)?;
    let cap = bound.as_ref();
    let get_report_id = cap.get_report_id_for_transport(device)?;
    let outgoing = device_reads::query_report(cap, command, transaction)?;
    let mut reading = None;
    let result = crate::feature_exchange::with_validated_exchange(
        device,
        crate::feature_exchange::with_protocol25_scope(
            ExchangeOptions::mutex_out(Duration::from_millis(cap.sleep_between_out_ms)),
            &cap.source_class,
        ),
        &deadline,
        |device, lease| {
            let mut last_error = anyhow::anyhow!("没有收到设备响应");
            'send: for out_attempt in 0..cap.max_retry_out {
                deadline()?;
                let sent = device.send_feature_at_collection_max_counted(&outgoing);
                if let Err(validation) = deadline() {
                    return Err(crate::backend::complete_native_io(sent, validation));
                }
                match sent
                    .and_then(|count| crate::backend::require_complete_send(count, outgoing.len()))
                {
                    Ok(()) => {}
                    Err(error) => {
                        if crate::backend::exchange_finalization_failed(&error) {
                            return Err(error);
                        }
                        last_error = error;
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
                    report[0] = get_report_id;
                    let received = device.get_feature_at_collection_max(&mut report);
                    if let Err(validation) = deadline() {
                        return Err(retain_failed_device_read(
                            crate::backend::complete_native_read(received, &report, validation),
                            cap,
                            command,
                            transaction,
                        ));
                    }
                    let count = match received {
                        Ok(count) => count,
                        Err(error) => {
                            // The current host turns a failed getFeatureReport into
                            // an empty response; middleware retries with a new OUT.
                            if crate::backend::exchange_finalization_failed(&error)
                                || crate::backend::native_read_bytes(&error).is_some()
                            {
                                return Err(retain_failed_device_read(
                                    error,
                                    cap,
                                    command,
                                    transaction,
                                ));
                            }
                            last_error = error;
                            break;
                        }
                    };
                    ensure!(count == cap.report_bytes, "设备查询返回长度不匹配：{count}");
                    match device_reads::decode_report(&report, cap, command, transaction)? {
                        ReadReply::Busy => last_error = anyhow::anyhow!("设备忙"),
                        ReadReply::Retry(reason) => {
                            last_error = anyhow::anyhow!(reason);
                            break;
                        }
                        ReadReply::Complete(value) => {
                            reading = Some(value);
                            break 'send;
                        }
                    }
                }
                if out_attempt + 1 < cap.max_retry_out {
                    lease.retry_release()?;
                }
            }
            if reading.is_none() {
                return Err(last_error).context("设备查询重试失败");
            }
            Ok(())
        },
    );
    if let Err(error) = result {
        return Err(match reading {
            Some(value) => error.context(CompletedDeviceRead { value }),
            None => error,
        });
    }
    reading.context("设备查询完成但未返回已解码响应")
}

pub fn read_receiver(
    device: &dyn FeatureTransport,
    selected: &super::receiver_capabilities::ReceiverCapability,
    transaction: u8,
    deadline: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<(u16, u8)>> {
    use super::receiver_protocol::{self as protocol, Reply};
    let outgoing = protocol::query_report(selected, transaction)?;
    let mut reading = None;
    let result = crate::feature_exchange::with_validated_exchange(
        device,
        crate::feature_exchange::with_protocol25_scope(
            ExchangeOptions::mutex_out(Duration::from_millis(selected.sleep_between_out_ms)),
            &selected.source_class,
        ),
        &deadline,
        |device, lease| {
            let mut last_error = anyhow::anyhow!("尚未取得连接状态");
            // Exact factory timing/retry parameters are source-derived, including the
            // factory's multiplier. Keep one transaction across retries, as source does.
            for out_attempt in 0..selected.max_retry_out {
                deadline()?;
                let sent = device.send_feature_at_collection_max_counted(&outgoing);
                if let Err(validation) = deadline() {
                    return Err(crate::backend::complete_native_io(sent, validation));
                }
                match sent
                    .and_then(|count| crate::backend::require_complete_send(count, outgoing.len()))
                {
                    Ok(()) => {}
                    Err(error) => {
                        if crate::backend::exchange_finalization_failed(&error) {
                            return Err(error);
                        }
                        last_error = error;
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
                    report[0] = selected.get_report_id();
                    let received = device.get_feature_at_collection_max(&mut report);
                    if let Err(validation) = deadline() {
                        return Err(retain_failed_receiver_read(
                            crate::backend::complete_native_read(received, &report, validation),
                            selected,
                            transaction,
                        ));
                    }
                    let received = match received {
                        Ok(count) => count,
                        Err(error) => {
                            if crate::backend::exchange_finalization_failed(&error)
                                || crate::backend::native_read_bytes(&error).is_some()
                            {
                                return Err(retain_failed_receiver_read(
                                    error,
                                    selected,
                                    transaction,
                                ));
                            }
                            last_error = error;
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
                            last_error = anyhow::anyhow!("设备忙，查询未完成");
                        }
                        Reply::Retry(reason) => {
                            last_error = anyhow::anyhow!(reason);
                            break;
                        }
                        Reply::Complete(devices) => {
                            reading = Some(devices);
                            return Ok(());
                        }
                    }
                }
                if out_attempt + 1 < selected.max_retry_out {
                    lease.retry_release()?;
                }
            }
            Err(last_error).context("接收器查询重试已耗尽")
        },
    );
    if let Err(error) = result {
        return Err(match reading {
            Some(devices) => error.context(CompletedReceiverRead { devices }),
            None => error,
        });
    }
    reading.context("接收器查询完成但未返回已解码设备清单")
}

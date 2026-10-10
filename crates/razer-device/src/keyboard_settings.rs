//! Current source-proved matrix brightness over the portable Feature transport.
//! Active brightness only: hardware-profile synchronization/task storage remain
//! caller responsibilities. The original active setter does not read back;
//! this module uses readback as an application confirmation policy.
use super::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, Deserialize)]
pub struct KeyboardSettingsCapability {
    pub product_id: u32,
    pub source_class: String,
    pub get_command: [u8; 3],
    pub set_command: [u8; 3],
    pub regions_command: [u8; 3],
    pub active_profile: u8,
    pub all_region: u8,
    pub fallback_region: u8,
    pub region_record_bytes: usize,
    pub transport: DeviceReadCapability,
}

pub fn capability(product_id: u32) -> Option<&'static KeyboardSettingsCapability> {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        products: Vec<KeyboardSettingsCapability>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let catalog: Catalog = serde_json::from_str(include_str!(
                "../../../assets/data/keyboard-settings-capabilities.json"
            ))
            .expect("current source keyboard brightness capabilities");
            assert_eq!(catalog.schema_version, 1);
            catalog
        })
        .products
        .iter()
        .find(|cap| cap.product_id == product_id)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyboardBrightness {
    pub profile_id: u8,
    /// Getter may resolve AllRegion to the first region returned by the device.
    pub region_id: u8,
    pub raw_value: u8,
    pub percent: u8,
}

#[derive(Debug, Serialize)]
pub struct KeyboardBrightnessWriteResult {
    pub requested_percent: u8,
    pub acknowledged: KeyboardBrightness,
    pub observed: KeyboardBrightness,
    /// Only true after a successful getter with the requested percentage.
    pub verified: bool,
}

/// Source setter's floor(percent / 100 * 255), with typed UI range validation.
pub fn encode_percent(percent: u8) -> anyhow::Result<u8> {
    ensure!(percent <= 100, "键盘亮度超出源码 UI 范围 0..100");
    Ok((u16::from(percent) * 255 / 100) as u8)
}

fn parse_brightness(data: &[u8], profile: u8, region: u8) -> anyhow::Result<KeyboardBrightness> {
    ensure!(data.len() >= 3, "键盘亮度响应字段截断");
    ensure!(
        data[0] == profile && data[1] == region,
        "键盘亮度响应配置或区域不匹配"
    );
    Ok(KeyboardBrightness {
        profile_id: data[0],
        region_id: data[1],
        raw_value: data[2],
        percent: (u16::from(data[2]) * 100).div_ceil(255) as u8,
    })
}

/// Execute one proven command on a retained transport and identity/deadline.
/// Report layout, retry timing and keyboard transaction namespace were already
/// source-gated individually in the embedded product transport capability.
fn exchange(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    let transport = &cap.transport;
    ensure!(
        transport.source_class == cap.source_class
            && transport.product_id == cap.product_id
            && transport.report_bytes == 91
            && transport.report_id == 0
            && cap.region_record_bytes == 5
            && payload.len() <= usize::from(command[0])
            && usize::from(command[0]) <= 80,
        "键盘亮度源码传输规格不匹配"
    );
    ensure!(
        transaction & 0xE0 == transport.transaction_prefix
            && transaction & 0x1F < transport.transaction_modulus,
        "键盘亮度事务不属于当前源码命名空间"
    );
    let mut outgoing = vec![0; transport.report_bytes];
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last_error = "没有收到键盘亮度响应".to_owned();
    for _ in 0..transport.max_retry_out {
        std::thread::sleep(Duration::from_millis(transport.sleep_between_out_ms));
        validate()?;
        let sent = device.send_feature(&outgoing);
        validate()?;
        if let Err(error) = sent {
            last_error = format!("{error:#}");
            continue;
        }
        std::thread::sleep(Duration::from_millis(transport.sleep_between_out_in_ms));
        for attempt in 0..transport.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(transport.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = vec![0; transport.report_bytes];
            let received = device.get_feature(&mut incoming);
            validate()?;
            let count = match received {
                Ok(count) => count,
                Err(error) => {
                    last_error = format!("{error:#}");
                    break;
                }
            };
            ensure!(
                count == 91 && incoming[0] == 0,
                "键盘亮度响应长度或 ReportID 不匹配"
            );
            let packet = &incoming[1..];
            if packet[1] != transaction || packet[6..8] != command[1..] {
                last_error = "键盘亮度事务或命令不匹配".into();
                break;
            }
            match packet[0] {
                1 => {
                    last_error = "键盘设备忙".into();
                    continue;
                }
                2 => {}
                0 | 3 | 4 => {
                    last_error = format!("键盘亮度响应状态 {}", packet[0]);
                    break;
                }
                state => anyhow::bail!("键盘亮度命令不受支持或返回未知状态 {state}"),
            }
            let len = usize::from(packet[5]);
            ensure!(len <= 80, "键盘亮度响应数据长度超出协议");
            return Ok(packet[8..8 + len].to_vec());
        }
    }
    anyhow::bail!("键盘亮度命令重试失败：{last_error}")
}

/// Read profile 1 using the original AllRegion getter resolution.
pub fn read(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardBrightness> {
    validate()?;
    let transaction = next_transaction()?;
    let regions = exchange(
        device,
        cap,
        cap.regions_command,
        &[],
        transaction,
        &validate,
    );
    // The original getter catches a rejected region query and selects region 1.
    // Identity/deadline cancellation must still stop before any fallback send.
    validate()?;
    let region = match regions {
        Ok(data) => data.first().copied().unwrap_or(cap.all_region),
        Err(_) => cap.fallback_region,
    };
    let data = exchange(
        device,
        cap,
        cap.get_command,
        &[cap.active_profile, region],
        next_transaction()?,
        &validate,
    )?;
    parse_brightness(&data, cap.active_profile, region)
}

/// Source active setter writes all regions. Readback is additional application
/// confirmation, never a claim that the original task reads back or persists.
pub fn apply(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    percent: u8,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardBrightnessWriteResult> {
    let raw = encode_percent(percent)?;
    validate()?;
    let data = exchange(
        device,
        cap,
        cap.set_command,
        &[cap.active_profile, cap.all_region, raw],
        next_transaction()?,
        &validate,
    )
    .context("尚未确认键盘亮度写入应答；设备可能已接受，请重新读取")?;
    let acknowledged = parse_brightness(&data, cap.active_profile, cap.all_region)?;
    validate().context("键盘亮度写入后会话或期限变化，尚未确认最终结果")?;
    let observed = read(device, cap, &mut next_transaction, &validate)
        .context("键盘亮度写入已应答，但回读失败，尚未确认最终结果")?;
    ensure!(
        observed.percent == percent,
        "键盘亮度回读与请求不符，写入结果尚未确认"
    );
    validate()?;
    Ok(KeyboardBrightnessWriteResult {
        requested_percent: percent,
        acknowledged,
        observed,
        verified: true,
    })
}

#[cfg(test)]
mod tests {
    // Current middleware/600 receipts in keyboard-settings-current-evidence.json:
    // getBrightness resolves AllRegion through the first stride-5 region record,
    // catches a rejected query with region 1, and retains region 0 for no records.
    // The setter floors percent/100*255; its parser ceils raw/255*100.
    // Strict identity/length validation and setter readback are application policy.
    use super::*;
    use serde_json::json;
    use std::{cell::Cell, collections::VecDeque, sync::Mutex};

    struct Reply {
        status: u8,
        data: Vec<u8>,
        count: usize,
        report_id: u8,
        declared_len: Option<u8>,
        wrong_transaction: bool,
    }

    fn reply(data: &[u8]) -> Reply {
        Reply {
            status: 2,
            data: data.to_vec(),
            count: 91,
            report_id: 0,
            declared_len: None,
            wrong_transaction: false,
        }
    }

    fn rejected() -> Reply {
        Reply {
            status: 3,
            ..reply(&[])
        }
    }

    struct MockTransport {
        sent: Mutex<Vec<Vec<u8>>>,
        replies: Mutex<VecDeque<Reply>>,
    }

    impl MockTransport {
        fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.into_iter().collect()),
            }
        }

        fn sent(&self) -> Vec<Vec<u8>> {
            self.sent.lock().unwrap().clone()
        }
    }

    impl FeatureTransport for MockTransport {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(report.to_vec());
            Ok(())
        }

        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .context("mock reply exhausted")?;
            let sent = self
                .sent
                .lock()
                .unwrap()
                .last()
                .context("mock has no sent report")?
                .clone();
            report.fill(0);
            report[0] = reply.report_id;
            report[1] = reply.status;
            report[2] = sent[2] ^ u8::from(reply.wrong_transaction);
            report[6] = reply.declared_len.unwrap_or(reply.data.len() as u8);
            report[7..9].copy_from_slice(&sent[7..9]);
            report[9..9 + reply.data.len()].copy_from_slice(&reply.data);
            Ok(reply.count)
        }

        fn metadata(&self) -> serde_json::Value {
            json!({"mock": true})
        }
    }

    fn cap() -> KeyboardSettingsCapability {
        let catalog: serde_json::Value = serde_json::from_str(include_str!(
            "../../../assets/data/keyboard-settings-capabilities.json"
        ))
        .unwrap();
        let mut cap: KeyboardSettingsCapability =
            serde_json::from_value(catalog["products"][0].clone()).unwrap();
        // No transport sleeps in unit tests. One retry makes error scripts exact.
        cap.transport.max_retry_in = 1;
        cap.transport.max_retry_out = 1;
        cap.transport.sleep_between_out_ms = 0;
        cap.transport.sleep_between_out_in_ms = 0;
        cap.transport.sleep_between_in_ms = 0;
        cap
    }

    fn transactions(cap: &KeyboardSettingsCapability) -> impl FnMut() -> anyhow::Result<u8> {
        let prefix = cap.transport.transaction_prefix;
        let modulus = cap.transport.transaction_modulus;
        let mut sequence = 0;
        move || {
            let transaction = prefix | sequence;
            sequence = (sequence + 1) % modulus;
            Ok(transaction)
        }
    }

    #[test]
    fn source_floor_and_ceil_round_trip_every_ui_percentage() {
        for percent in 0..=100 {
            let raw = encode_percent(percent).unwrap();
            assert_eq!(raw, (u16::from(percent) * 255 / 100) as u8);
            let parsed = parse_brightness(&[1, 0, raw], 1, 0).unwrap();
            assert_eq!(parsed.percent, percent);
        }
        assert_eq!(encode_percent(1).unwrap(), 2);
        assert_eq!(parse_brightness(&[1, 0, 1], 1, 0).unwrap().percent, 1);
        assert_eq!(parse_brightness(&[1, 0, 254], 1, 0).unwrap().percent, 100);
        assert!(encode_percent(101).is_err());
    }

    #[test]
    fn all_region_read_uses_first_region_record_in_device_order() {
        let cap = cap();
        let device = MockTransport::new([
            reply(&[5, 60, 1, 20, 6, 1, 30, 2, 1, 1]),
            reply(&[1, 5, 127]),
        ]);
        let result = read(&device, &cap, transactions(&cap), || Ok(())).unwrap();
        assert_eq!(
            (result.region_id, result.raw_value, result.percent),
            (5, 127, 50)
        );
        let sent = device.sent();
        assert_eq!(&sent[0][6..9], &cap.regions_command);
        assert!(sent[0][9..89].iter().all(|byte| *byte == 0));
        assert_eq!(&sent[1][6..9], &cap.get_command);
        assert_eq!(&sent[1][9..12], &[1, 5, 0]);
        assert_eq!(sent[1][2], sent[0][2] + 1);
    }

    #[test]
    fn empty_region_list_keeps_all_region_but_rejected_query_falls_back_to_one() {
        let cap = cap();
        for (regions, region) in [(reply(&[]), 0), (rejected(), 1)] {
            let device = MockTransport::new([regions, reply(&[1, region, 255])]);
            let result = read(&device, &cap, transactions(&cap), || Ok(())).unwrap();
            assert_eq!((result.region_id, result.percent), (region, 100));
            assert_eq!(&device.sent()[1][9..11], &[1, region]);
        }
    }

    #[test]
    fn setter_sends_source_bytes_and_only_verifies_after_successful_readback() {
        let cap = cap();
        let device = MockTransport::new([
            reply(&[1, 0, 127]),
            reply(&[5, 60, 1, 20, 6]),
            reply(&[1, 5, 127]),
        ]);
        let result = apply(&device, &cap, 50, transactions(&cap), || Ok(())).unwrap();
        assert!(result.verified);
        assert_eq!(result.requested_percent, 50);
        assert_eq!(result.acknowledged.region_id, 0);
        assert_eq!(result.observed.region_id, 5);
        let sent = device.sent();
        assert_eq!(sent.len(), 3);
        assert_eq!(&sent[0][6..9], &[3, 15, 4]);
        assert_eq!(&sent[0][9..12], &[1, 0, 127]);
        assert_eq!(&sent[1][6..9], &[80, 15, 128]);
        assert_eq!(&sent[2][6..9], &[3, 15, 132]);
        for report in sent {
            assert_eq!(report.len(), 91);
            assert_eq!(report[0], 0);
            assert_eq!(
                report[89],
                report[3..89].iter().fold(0, |sum, byte| sum ^ byte)
            );
            assert_eq!(report[90], 0);
        }
    }

    #[test]
    fn rejected_truncated_or_wrong_identity_setter_never_starts_readback() {
        let cap = cap();
        for setter in [
            rejected(),
            reply(&[1, 0]),
            reply(&[2, 0, 127]),
            reply(&[1, 5, 127]),
        ] {
            let device = MockTransport::new([setter]);
            assert!(apply(&device, &cap, 50, transactions(&cap), || Ok(())).is_err());
            assert_eq!(device.sent().len(), 1);
        }
    }

    #[test]
    fn failed_partial_or_different_readback_never_returns_verified() {
        let cap = cap();
        for getter in [
            rejected(),
            reply(&[1, 5]),
            reply(&[1, 5, 124]),
            reply(&[1, 1, 127]),
        ] {
            let device =
                MockTransport::new([reply(&[1, 0, 127]), reply(&[5, 60, 1, 20, 6]), getter]);
            assert!(apply(&device, &cap, 50, transactions(&cap), || Ok(())).is_err());
            assert_eq!(device.sent().len(), 3);
        }
    }

    #[test]
    fn getter_rejects_short_report_invalid_id_and_oversized_payload() {
        let cap = cap();
        for getter in [
            Reply {
                count: 90,
                ..reply(&[1, 5, 127])
            },
            Reply {
                report_id: 1,
                ..reply(&[1, 5, 127])
            },
            Reply {
                declared_len: Some(81),
                ..reply(&[1, 5, 127])
            },
            Reply {
                wrong_transaction: true,
                ..reply(&[1, 5, 127])
            },
        ] {
            let device = MockTransport::new([reply(&[5, 60, 1, 20, 6]), getter]);
            assert!(read(&device, &cap, transactions(&cap), || Ok(())).is_err());
            assert_eq!(device.sent().len(), 2);
        }
    }

    #[test]
    fn invalid_percentage_payload_or_transaction_is_rejected_before_send() {
        let cap = cap();
        let device = MockTransport::new([]);
        assert!(apply(&device, &cap, 101, transactions(&cap), || Ok(())).is_err());
        assert!(
            exchange(
                &device,
                &cap,
                cap.set_command,
                &[1, 0, 127, 0],
                cap.transport.transaction_prefix,
                &|| Ok(())
            )
            .is_err()
        );
        assert!(
            exchange(
                &device,
                &cap,
                cap.set_command,
                &[1, 0, 127],
                cap.transport.transaction_prefix | cap.transport.transaction_modulus,
                &|| Ok(())
            )
            .is_err()
        );
        assert!(
            exchange(
                &device,
                &cap,
                [81, 15, 4],
                &[],
                cap.transport.transaction_prefix,
                &|| Ok(())
            )
            .is_err()
        );
        assert!(device.sent().is_empty());
    }

    #[test]
    fn canceled_region_query_never_sends_fallback() {
        let cap = cap();
        let device = MockTransport::new([rejected()]);
        let validations = Cell::new(0);
        let result = read(&device, &cap, transactions(&cap), || {
            let count = validations.get() + 1;
            validations.set(count);
            // Cancel at the explicit post-region-query check, after its error.
            ensure!(count < 6, "mock cancellation");
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(device.sent().len(), 1);
    }
}

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

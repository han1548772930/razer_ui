//! Current source-proved keyboard brightness over the portable Feature transport.
//! Active brightness only: hardware-profile synchronization/task storage remain
//! caller responsibilities. The original active setter does not read back;
//! this module uses readback as an application confirmation policy.
use super::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

#[derive(Clone, Debug, Deserialize)]
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
    pub hardware_profile_slots: u8,
    pub transport: DeviceReadCapability,
}

struct KeyboardSettingsDescriptor {
    product_id: u32,
    claim_interface: u8,
    claim_interfaces_by_pid: &'static [(u32, u8)],
    hardware_profile_slots: u8,
}

include!("keyboard_settings_data_generated.rs");
include!("keyboard_brightness_current_generated.rs");

// Current 515 MW main.8342a5d11f2dd0d29680.js registers
// rzDevice25LedMatrix with BacklightLED and claimInterface 2. This capability
// covers active brightness only and does not advertise hardware profile slots.
// Unlike the Chroma class this inherits legacy LED ceil/floor brightness.
const BLACKWIDOW_CHROMA: KeyboardSettingsDescriptor = KeyboardSettingsDescriptor {
    product_id: 515,
    claim_interface: 2,
    claim_interfaces_by_pid: &[],
    hardware_profile_slots: 0,
};

pub fn capability(product_id: u32) -> Option<&'static KeyboardSettingsCapability> {
    // DeviceReadCapability is also consumed by retained discovery/service
    // adapters. Materialize it once from typed, audited Rust descriptors.
    static CATALOG: OnceLock<Vec<KeyboardSettingsCapability>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            AUDITED_KEYBOARDS
                .iter()
                .chain(CURRENT_AUDITED_KEYBOARDS)
                .chain(std::iter::once(&BLACKWIDOW_CHROMA))
                .map(|descriptor| KeyboardSettingsCapability {
                    product_id: descriptor.product_id,
                    source_class: if descriptor.product_id == 515 {
                        "rzDevice25LedMatrix"
                    } else {
                        "rzDevice25"
                    }
                    .into(),
                    get_command: if descriptor.product_id == 515 {
                        [3, 3, 131]
                    } else {
                        [3, 15, 132]
                    },
                    set_command: if descriptor.product_id == 515 {
                        [3, 3, 3]
                    } else {
                        [3, 15, 4]
                    },
                    regions_command: if descriptor.product_id == 515 {
                        [80, 3, 137]
                    } else {
                        [80, 15, 128]
                    },
                    active_profile: 1,
                    // Both callers explicitly select BacklightLED (5); 631
                    // uses the Chroma command and rounding, 515 legacy LED.
                    all_region: if matches!(descriptor.product_id, 515 | 631) {
                        5
                    } else {
                        0
                    },
                    fallback_region: 1,
                    region_record_bytes: if descriptor.product_id == 515 { 1 } else { 5 },
                    hardware_profile_slots: descriptor.hardware_profile_slots,
                    transport: DeviceReadCapability {
                        product_id: descriptor.product_id,
                        source_class: if descriptor.product_id == 515 {
                            "rzDevice25LedMatrix"
                        } else {
                            "rzDevice25"
                        }
                        .into(),
                        direct_pids: vec![descriptor.product_id],
                        vendor_id: 0x1532,
                        claim_interface: if descriptor.claim_interfaces_by_pid.is_empty() {
                            descriptor.claim_interface
                        } else {
                            descriptor
                                .claim_interfaces_by_pid
                                .iter()
                                .find(|(pid, _)| *pid == descriptor.product_id)
                                .expect("audited wired PID must exist in the source interface map")
                                .1
                        },
                        // This capability exposes only the wired physical PID.
                        // The source descriptor has already selected that PID's
                        // interface; keep its radio/BLE map out of this binding.
                        claim_interfaces_by_pid: BTreeMap::new(),
                        physical_transports_by_pid: Default::default(),
                        report_id: 0,
                        report_bytes: 91,
                        transaction_prefix: 0,
                        // rzDevice starts at zero and returns transactionId++:
                        // actual wired IDs are 0..30 before reset to zero.
                        transaction_modulus: 31,
                        queries: Vec::new(),
                        polling_codes: BTreeMap::new(),
                        polling_value_offset: None,
                        polling_default_hz: None,
                        polling_physical_product_id: None,
                        charging_codes: BTreeMap::new(),
                        max_retry_in: 10,
                        max_retry_out: 20,
                        sleep_between_out_ms: 5,
                        sleep_between_out_in_ms: 5,
                        sleep_between_in_ms: 5,
                    },
                })
                .collect()
        })
        .iter()
        .find(|cap| cap.product_id == product_id)
}

/// Actual wired rzDevice25's post-incremented transaction ID, 0..30.
/// The shared retained session allocator supplies the same 0..30 counter.
pub fn wire_transaction(sequence: u8) -> anyhow::Result<u8> {
    ensure!(sequence < 31, "键盘有线事务计数超出源代码范围 0..30");
    Ok(sequence)
}

pub fn receiver_capability(
    product_id: u32,
    physical_product_id: u32,
    peer_product_id: u32,
) -> anyhow::Result<(
    KeyboardSettingsCapability,
    crate::receiver_session::ReceiverSession,
)> {
    let session = crate::receiver_session::ReceiverSession::keyboard(
        product_id,
        physical_product_id,
        peer_product_id,
    )?;
    let mut settings = capability(product_id)
        .context("Keyboard brightness source is absent")?
        .clone();
    settings.transport = session.transport().clone();
    Ok((settings, session))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyboardRegion {
    pub region_id: u8,
    pub update_rate: u8,
    pub data_type: u8,
    pub array_x: u8,
    pub array_y: u8,
}

fn parse_regions(data: &[u8]) -> anyhow::Result<Vec<KeyboardRegion>> {
    ensure!(data.len() % 5 == 0, "键盘区域列表响应的五字节记录截断");
    Ok(data
        .chunks_exact(5)
        .map(|record| KeyboardRegion {
            region_id: record[0],
            update_rate: record[1],
            data_type: record[2],
            array_x: record[3],
            array_y: record[4],
        })
        .collect())
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

fn parse_device_brightness(
    data: &[u8],
    cap: &KeyboardSettingsCapability,
    region: u8,
) -> anyhow::Result<KeyboardBrightness> {
    let mut reading = parse_brightness(data, cap.active_profile, region)?;
    if cap.product_id == BLACKWIDOW_CHROMA.product_id {
        // MW rzDevice25 module 34546 g: floor(raw / 255 * 100).
        reading.percent = (u16::from(reading.raw_value) * 100 / 255) as u8;
    }
    Ok(reading)
}

/// Execute one proven command on a retained transport and identity/deadline.
/// Report layout, retry timing and keyboard transaction namespace were already
/// source-gated individually in the embedded product transport capability.
pub(crate) fn exchange(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    command: [u8; 3],
    payload: &[u8],
    sequence: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    let transport = &cap.transport;
    let audited = capability(cap.product_id).context("键盘型号未通过当前有线亮度源码审计")?;
    let wired = transport.source_class == audited.transport.source_class
        && transport.direct_pids.as_slice() == [cap.product_id]
        && transport.claim_interface == audited.transport.claim_interface
        && transport.claim_interfaces_by_pid == audited.transport.claim_interfaces_by_pid
        && transport.report_id == 0
        && transport.transaction_prefix == 0
        && transport.transaction_modulus == 31;
    let receiver = crate::receiver_session::is_bound_master(transport, true);
    ensure!(
        cap.source_class == audited.source_class
            && cap.get_command == audited.get_command
            && cap.set_command == audited.set_command
            && cap.regions_command == audited.regions_command
            && cap.active_profile == audited.active_profile
            && cap.all_region == audited.all_region
            && cap.fallback_region == audited.fallback_region
            && cap.region_record_bytes == audited.region_record_bytes
            && cap.hardware_profile_slots == audited.hardware_profile_slots
            && transport.vendor_id == audited.transport.vendor_id
            && (wired || receiver)
            && transport.max_retry_in > 0
            && transport.max_retry_out > 0,
        "键盘亮度型号的固定协议或 factory 参数偏离当前审计能力"
    );
    ensure!(
        transport.product_id == cap.product_id
            && transport.report_bytes == 91
            && cap.region_record_bytes == audited.region_record_bytes
            && payload.len() <= usize::from(command[0])
            && usize::from(command[0]) <= 80,
        "键盘亮度源码传输规格不匹配"
    );
    ensure!(
        sequence & 0xE0 == transport.transaction_prefix
            && sequence & 0x1F < transport.transaction_modulus,
        "键盘亮度事务不属于当前源码命名空间"
    );
    let transaction = sequence;
    let mut outgoing = vec![0; transport.report_bytes];
    outgoing[0] = transport.report_id;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last_error = "没有收到键盘亮度响应".to_owned();
    for _ in 0..transport.max_retry_out {
        std::thread::sleep(Duration::from_millis(transport.sleep_between_out_ms));
        validate()?;
        let sent = device.send_feature_at_collection_max(&outgoing);
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
            incoming[0] = transport.get_report_id_for_transport(device)?;
            let received = device.get_feature_at_collection_max(&mut incoming);
            validate()?;
            let count = match received {
                Ok(count) => count,
                Err(error) => {
                    last_error = format!("{error:#}");
                    break;
                }
            };
            ensure!(count == 91, "键盘亮度响应长度不匹配");
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

/// Query the source's five-byte region records in device order.
/// Transaction callbacks return the session's 0..30 counter, as for read/apply.
pub fn read_regions(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<KeyboardRegion>> {
    // Legacy LED ID lists contain IDs only; they cannot truthfully populate
    // Chroma's update rate, data type or matrix dimensions.
    ensure!(
        cap.region_record_bytes == 5,
        "此键盘返回 LED ID 列表而非 Chroma 矩阵区域记录"
    );
    validate()?;
    let data = exchange(
        device,
        cap,
        cap.regions_command,
        &[],
        next_transaction()?,
        &validate,
    )?;
    validate()?;
    parse_regions(&data)
}

/// Read active brightness using the original caller's LED/region selection.
/// Explicit BacklightLED callers query that region directly; AllRegion callers
/// resolve the first region. The callback returns the selected sender's wire ID.
pub fn read(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardBrightness> {
    validate()?;
    if cap.all_region != 0 {
        let data = exchange(
            device,
            cap,
            cap.get_command,
            &[cap.active_profile, cap.all_region],
            next_transaction()?,
            &validate,
        )?;
        validate()?;
        return parse_device_brightness(&data, cap, cap.all_region);
    }
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
        Ok(data) => parse_regions(&data)?
            .first()
            .map_or(cap.all_region, |record| record.region_id),
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

/// Source active setter targets BacklightLED on 515 and all regions on Chroma.
/// Readback is additional application confirmation, never a claim that the
/// original task reads back or persists.
pub fn apply(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    percent: u8,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardBrightnessWriteResult> {
    let raw = if cap.product_id == BLACKWIDOW_CHROMA.product_id {
        encode_percent(percent)?; // The UI range is shared, the wire rounding is not.
        (u16::from(percent) * 255).div_ceil(100) as u8
    } else {
        encode_percent(percent)?
    };
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
    let acknowledged = parse_device_brightness(&data, cap, cap.all_region)?;
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
    // Per-product current receipts in keyboard-brightness-expanded-current-source.json:
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
        get_ids: Mutex<Vec<u8>>,
        collection_sends: Cell<usize>,
        collection_gets: Cell<usize>,
    }

    impl MockTransport {
        fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.into_iter().collect()),
                get_ids: Mutex::new(Vec::new()),
                collection_sends: Cell::new(0),
                collection_gets: Cell::new(0),
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
            self.get_ids.lock().unwrap().push(report[0]);
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

        fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
            self.collection_sends.set(self.collection_sends.get() + 1);
            self.send_feature(report)
        }

        fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.collection_gets.set(self.collection_gets.get() + 1);
            self.get_feature(report)
        }

        fn metadata(&self) -> serde_json::Value {
            json!({"mock": true})
        }
    }

    fn cap() -> KeyboardSettingsCapability {
        cap_for(600)
    }

    fn cap_for(product_id: u32) -> KeyboardSettingsCapability {
        let mut cap = capability(product_id).unwrap().clone();
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
    fn pro_type_ultra_uses_chroma_backlight_without_querying_regions() {
        let cap = cap_for(631);
        assert_eq!(cap.transport.claim_interface, 2);
        assert_eq!(cap.hardware_profile_slots, 0);
        assert_eq!(cap.all_region, 5);
        for percent in [0, 1, 30, 50, 99, 100] {
            let raw = encode_percent(percent).unwrap();
            let device = MockTransport::new([reply(&[1, 5, raw]), reply(&[1, 5, raw])]);
            let result = apply(&device, &cap, percent, transactions(&cap), || Ok(())).unwrap();
            assert!(result.verified);
            assert_eq!(result.observed.percent, percent);
            let sent = device.sent();
            assert_eq!(sent.len(), 2);
            assert_eq!(&sent[0][6..12], &[3, 15, 4, 1, 5, raw]);
            assert_eq!(&sent[1][6..12], &[3, 15, 132, 1, 5, 0]);
            assert_eq!(*device.get_ids.lock().unwrap(), [2, 2]);
        }
    }

    #[test]
    fn blackwidow_chroma_active_brightness_uses_legacy_packets_for_every_percentage() {
        let cap = cap_for(515);
        assert_eq!(cap.source_class, "rzDevice25LedMatrix");
        assert_eq!(cap.transport.claim_interface, 2);
        assert_eq!(cap.transport.direct_pids, [515]);
        for percent in 0u8..=100 {
            let raw = (u16::from(percent) * 255).div_ceil(100) as u8;
            let device = MockTransport::new([reply(&[1, 5, raw]), reply(&[1, 5, raw])]);
            let result = apply(&device, &cap, percent, transactions(&cap), || Ok(())).unwrap();
            assert!(result.verified);
            assert_eq!(result.observed.percent, percent);
            assert_eq!(result.observed.raw_value, raw);
            let sent = device.sent();
            assert_eq!(sent.len(), 2, "BacklightLED never queries a region list");
            assert_eq!(&sent[0][6..12], &[3, 3, 3, 1, 5, raw]);
            assert_eq!(&sent[1][6..12], &[3, 3, 131, 1, 5, 0]);
            assert_eq!((sent[0][2], sent[1][2]), (0, 1));
            assert_eq!(
                sent[0][89],
                sent[0][3..89].iter().fold(0, |sum, byte| sum ^ byte)
            );
        }
        let device = MockTransport::new([]);
        assert!(apply(&device, &cap, 101, transactions(&cap), || Ok(())).is_err());
        assert!(device.sent().is_empty());
    }

    #[test]
    fn blackwidow_chroma_read_preserves_floor_rounding_and_led_identity() {
        let cap = cap_for(515);
        for (raw, percent) in [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 1),
            (127, 49),
            (128, 50),
            (254, 99),
            (255, 100),
        ] {
            let device = MockTransport::new([reply(&[1, 5, raw])]);
            let observed = read(&device, &cap, transactions(&cap), || Ok(())).unwrap();
            assert_eq!(observed.percent, percent);
            assert_eq!(observed.region_id, 5);
            assert_eq!(device.sent().len(), 1);
        }
        for data in [&[1, 0, 128][..], &[0, 5, 128], &[1, 5]] {
            let device = MockTransport::new([reply(data)]);
            assert!(read(&device, &cap, transactions(&cap), || Ok(())).is_err());
        }
    }

    #[test]
    fn blackwidow_chroma_acknowledgment_does_not_fabricate_readback_or_ignore_cancel() {
        let cap = cap_for(515);
        // Native setter acknowledgment alone does not confirm the independent
        // Rust readback. Failure does not rewrite the optimistic UI draft.
        for readback in [rejected(), reply(&[1, 5, 3])] {
            let device = MockTransport::new([reply(&[1, 5, 128]), readback]);
            assert!(apply(&device, &cap, 50, transactions(&cap), || Ok(())).is_err());
            assert_eq!(device.sent().len(), 2);
        }
        let device = MockTransport::new([reply(&[1, 5, 128])]);
        let validate = || {
            ensure!(
                !device.replies.lock().unwrap().is_empty(),
                "simulated connection change"
            );
            Ok(())
        };
        assert!(apply(&device, &cap, 50, transactions(&cap), validate).is_err());
        assert_eq!(device.sent().len(), 1, "cancellation must prevent readback");
    }

    #[test]
    fn blackwidow_chroma_cannot_be_retargeted_to_chroma_command_or_another_interface() {
        let device = MockTransport::new([]);
        let mut forged = cap_for(515);
        forged.get_command = [3, 15, 132];
        assert!(read(&device, &forged, transactions(&forged), || Ok(())).is_err());
        let mut forged = cap_for(515);
        forged.transport.claim_interface = 3;
        assert!(read(&device, &forged, transactions(&forged), || Ok(())).is_err());
        let mut forged = cap_for(515);
        forged.all_region = 0;
        assert!(apply(&device, &forged, 50, transactions(&forged), || Ok(())).is_err());
        assert!(device.sent().is_empty());
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
    fn getter_rejects_short_report_wrong_transaction_and_oversized_payload() {
        let cap = cap();
        for getter in [
            Reply {
                count: 90,
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
    fn wired_brightness_selects_the_source_get_interface_and_discards_returned_prefix() {
        // Complete current 600/602 factories select interface 3/2. The host's
        // SET uses explicit reportId=0, GET falls back to that interface, and
        // slice(1) discards the returned prefix without comparing it.
        for (product, interface) in [(600, 3), (602, 2)] {
            let cap = cap_for(product);
            let device = MockTransport::new([
                Reply {
                    report_id: 17,
                    ..reply(&[5, 60, 1, 20, 6])
                },
                Reply {
                    report_id: 29,
                    ..reply(&[1, 5, 127])
                },
            ]);
            let observed = read(&device, &cap, transactions(&cap), || Ok(())).unwrap();
            assert_eq!(observed.raw_value, 127);
            assert_eq!(*device.get_ids.lock().unwrap(), [interface, interface]);
            assert!(device.sent().iter().all(|report| report[0] == 0));
            assert_eq!(device.collection_sends.get(), 2);
            assert_eq!(device.collection_gets.get(), 2);
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

    #[test]
    fn every_audited_keyboard_uses_its_actual_wired_factory_interface_and_packets() {
        for (pid, interface, slots) in [
            (600, 3, 4),
            (602, 2, 4),
            (658, 3, 4),
            (664, 3, 4),
            (678, 3, 5),
            (679, 3, 5),
            (688, 3, 5),
            (697, 3, 4),
            (708, 3, 1),
            (716, 3, 4),
            (717, 3, 4),
            (724, 3, 4),
            (727, 3, 4),
            (756, 3, 5),
        ] {
            let production = capability(pid).unwrap();
            assert_eq!(production.source_class, "rzDevice25");
            assert_eq!(production.transport.direct_pids, [pid]);
            assert_eq!(
                production.transport.claim_interface_for(pid).unwrap(),
                interface
            );
            assert_eq!(production.hardware_profile_slots, slots);
            assert_eq!(production.transport.transaction_prefix, 0);
            assert_eq!(production.transport.transaction_modulus, 31);
            assert_eq!(production.transport.sleep_between_out_ms, 5);
            assert_eq!(production.transport.sleep_between_out_in_ms, 5);
            assert_eq!(production.transport.sleep_between_in_ms, 5);
            assert_eq!(production.transport.max_retry_in, 10);
            assert_eq!(production.transport.max_retry_out, 20);
            let cap = cap_for(pid);
            let device = MockTransport::new([
                reply(&[1, 0, 127]),
                reply(&[7, 60, 1, 20, 6]),
                reply(&[1, 7, 127]),
            ]);
            let result = apply(&device, &cap, 50, transactions(&cap), || Ok(())).unwrap();
            assert!(result.verified);
            let sent = device.sent();
            assert_eq!(&sent[0][6..12], &[3, 15, 4, 1, 0, 127]);
            assert_eq!(&sent[1][6..9], &[80, 15, 128]);
            assert_eq!(&sent[2][6..12], &[3, 15, 132, 1, 7, 0]);
            assert_eq!(
                sent.iter().map(|report| report[2]).collect::<Vec<_>>(),
                [0, 1, 2]
            );
        }
    }

    #[test]
    fn wired_transaction_postincrement_uses_zero_through_thirty_then_wraps() {
        let cap = cap();
        let device = MockTransport::new((0..63).map(|_| reply(&[])));
        let mut next = transactions(&cap);
        for _ in 0..63 {
            assert!(
                read_regions(&device, &cap, &mut next, || Ok(()))
                    .unwrap()
                    .is_empty()
            );
        }
        let sent = device.sent();
        let expected: Vec<_> = (0..63).map(|index| (index % 31) as u8).collect();
        assert_eq!(
            sent.iter().map(|report| report[2]).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(wire_transaction(0).unwrap(), 0);
        assert_eq!(wire_transaction(30).unwrap(), 30);
        assert!(wire_transaction(31).is_err());
        assert!(wire_transaction(128).is_err());
    }

    #[test]
    fn receiver_ble_and_unreviewed_products_do_not_inherit_wired_capability() {
        assert_eq!(AUDITED_KEYBOARDS.len(), 12);
        for unsupported in [
            0, 603, 604, 625, 626, 662, 663, 698, 699, 706, 707, 713, 714, 718, 722, 723, 725, 726,
            730, 691, 632, 635, 656, 657, 757, 758,
        ] {
            assert!(
                capability(unsupported).is_none(),
                "unsupported PID {unsupported}"
            );
        }
        let device = MockTransport::new([]);
        let mut invalid = cap();
        invalid.transport.direct_pids.push(625);
        assert!(read_regions(&device, &invalid, || Ok(0), || Ok(())).is_err());
        invalid = cap();
        invalid.source_class = "rzDevice25DualLinkKeyboard".into();
        invalid.transport.source_class = invalid.source_class.clone();
        invalid.transport.transaction_prefix = 128;
        assert!(read_regions(&device, &invalid, || Ok(0), || Ok(())).is_err());
        assert!(device.sent().is_empty());
    }

    #[test]
    fn forged_public_capability_cannot_expand_product_headers_or_factory_parameters() {
        let device = MockTransport::new([]);
        let mut variants = Vec::new();
        let mut forged = cap();
        forged.product_id = 756;
        forged.transport.product_id = 756;
        forged.transport.direct_pids = vec![756];
        variants.push(forged);
        let mut forged = cap();
        forged.get_command = [3, 15, 133];
        variants.push(forged);
        let mut forged = cap();
        forged.set_command = [3, 15, 5];
        variants.push(forged);
        let mut forged = cap();
        forged.regions_command = [80, 15, 129];
        variants.push(forged);
        let mut forged = cap();
        forged.active_profile = 2;
        variants.push(forged);
        let mut forged = cap();
        forged.all_region = 5;
        variants.push(forged);
        let mut forged = cap();
        forged.fallback_region = 5;
        variants.push(forged);
        let mut forged = cap();
        forged.hardware_profile_slots = 8;
        variants.push(forged);
        let mut forged = cap();
        forged.transport.vendor_id = 0x1234;
        variants.push(forged);
        let mut forged = cap();
        forged.transport.claim_interface = 2;
        variants.push(forged);
        let mut forged = cap_for(664);
        forged
            .transport
            .claim_interfaces_by_pid
            .insert("664".into(), 2);
        variants.push(forged);
        for forged in variants {
            assert!(read_regions(&device, &forged, || Ok(0), || Ok(())).is_err());
        }
        assert!(device.sent().is_empty());
    }

    #[test]
    fn region_query_preserves_all_five_source_fields_and_unknown_numeric_region() {
        let cap = cap();
        let device = MockTransport::new([reply(&[5, 60, 1, 20, 6, 222, 30, 9, 1, 2])]);
        let regions = read_regions(&device, &cap, transactions(&cap), || Ok(())).unwrap();
        assert_eq!(
            regions,
            [
                KeyboardRegion {
                    region_id: 5,
                    update_rate: 60,
                    data_type: 1,
                    array_x: 20,
                    array_y: 6
                },
                KeyboardRegion {
                    region_id: 222,
                    update_rate: 30,
                    data_type: 9,
                    array_x: 1,
                    array_y: 2
                },
            ]
        );
        assert_eq!(device.sent().len(), 1);
    }

    #[test]
    fn successful_but_truncated_regions_are_errors_and_never_trigger_fallback_getter() {
        let cap = cap();
        for length in [1, 2, 3, 4, 6, 7, 8, 9] {
            let device = MockTransport::new([reply(&vec![5; length])]);
            assert!(read(&device, &cap, transactions(&cap), || Ok(())).is_err());
            assert_eq!(device.sent().len(), 1);
        }
    }

    #[test]
    fn busy_response_retries_input_without_resending_or_advancing_transaction() {
        let mut cap = cap();
        cap.transport.max_retry_in = 2;
        let device = MockTransport::new([
            Reply {
                status: 1,
                ..reply(&[])
            },
            reply(&[]),
        ]);
        assert!(
            read_regions(&device, &cap, transactions(&cap), || Ok(()))
                .unwrap()
                .is_empty()
        );
        assert_eq!(device.sent().len(), 1);
        assert_eq!(device.sent()[0][2], 0);
    }

    #[test]
    fn cancellation_after_ack_stops_before_any_readback_or_success_result() {
        let cap = cap();
        let device = MockTransport::new([reply(&[1, 0, 127])]);
        let validations = Cell::new(0);
        assert!(
            apply(&device, &cap, 50, transactions(&cap), || {
                let count = validations.get() + 1;
                validations.set(count);
                ensure!(count < 6, "mock cancellation after ACK");
                Ok(())
            })
            .is_err()
        );
        assert_eq!(device.sent().len(), 1);
    }
}

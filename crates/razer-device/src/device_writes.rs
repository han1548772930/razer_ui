//! Source-proved typed Razer setters over the same host-neutral Feature contract.
//! Never accepts an arbitrary report/export, profile ID or saved device identity.
use super::{
    backend::FeatureTransport,
    device_query,
    device_reads::{DeviceReadCapability, DeviceReadKind, DeviceReadValue, ReadCommand},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeviceWriteSetting {
    /// Current active X/Y DPI; does not write the persistent stage table.
    Dpi { x: u16, y: u16 },
    /// The source caller's high-speed USB profile; not BLE/wireless polling.
    Polling { hz: u32 },
    /// Original setTimeToSleep units, before product-specific UI conversion.
    Idle { raw_time: u16 },
}

impl DeviceWriteSetting {
    pub fn read_kind(&self) -> DeviceReadKind {
        match self {
            Self::Dpi { .. } => DeviceReadKind::Dpi,
            Self::Polling { .. } => DeviceReadKind::Polling,
            Self::Idle { .. } => DeviceReadKind::Idle,
        }
    }

    fn matches(&self, value: &DeviceReadValue) -> bool {
        match (self, value) {
            (
                Self::Dpi { x, y },
                DeviceReadValue::Dpi {
                    x: actual_x,
                    y: actual_y,
                },
            ) => x == actual_x && y == actual_y,
            (Self::Polling { hz }, DeviceReadValue::Polling { hz: actual }) => hz == actual,
            (Self::Idle { raw_time }, DeviceReadValue::Idle { raw_time: actual }) => {
                raw_time == actual
            }
            _ => false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct WriteCommand {
    pub kind: DeviceReadKind,
    pub method: String,
    pub command: [u8; 3],
    pub selector: u8,
    pub read_kind: DeviceReadKind,
    pub min_response_bytes: usize,
}

#[derive(Debug, Deserialize)]
pub struct DeviceWriteCapability {
    pub product_id: u32,
    pub source_class: String,
    pub min_dpi: u16,
    pub max_dpi: u16,
    pub polling_codes: BTreeMap<String, u8>,
    pub writes: Vec<WriteCommand>,
}

pub fn capability(product_id: u32) -> Option<&'static DeviceWriteCapability> {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        products: Vec<DeviceWriteCapability>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let catalog: Catalog = serde_json::from_str(include_str!(
                "../../../assets/data/device-write-capabilities.json"
            ))
            .expect("source-derived device write capabilities");
            assert_eq!(catalog.schema_version, 1);
            catalog
        })
        .products
        .iter()
        .find(|cap| cap.product_id == product_id)
}

pub fn prepare(
    cap: &DeviceWriteCapability,
    setting: &DeviceWriteSetting,
) -> anyhow::Result<ReadCommand> {
    let command = cap
        .writes
        .iter()
        .find(|command| command.kind == setting.read_kind())
        .context("该设置没有当前源核实的写入能力")?;
    ensure!(
        command.read_kind == setting.read_kind(),
        "源写入解析器不匹配"
    );
    let mut payload = vec![command.selector];
    match setting {
        DeviceWriteSetting::Dpi { x, y } => {
            ensure!(
                (cap.min_dpi..=cap.max_dpi).contains(x) && (cap.min_dpi..=cap.max_dpi).contains(y),
                "DPI 超出当前产品源码范围"
            );
            payload.extend_from_slice(&x.to_be_bytes());
            payload.extend_from_slice(&y.to_be_bytes());
            // Current setDpiLevel(0, x, y) caller leaves helper's dpiZ default 0.
            payload.extend_from_slice(&0u16.to_be_bytes());
        }
        DeviceWriteSetting::Polling { hz } => {
            let code = cap
                .polling_codes
                .get(&format!("RATE_{hz}Hz"))
                .context("回报率没有当前源码枚举编码")?;
            payload.push(*code);
        }
        DeviceWriteSetting::Idle { raw_time } => {
            // Unlike DPI/polling, the source has no selector byte. Its helper
            // sends big-endian timeToSleep in the two payload bytes.
            payload = raw_time.to_be_bytes().to_vec();
        }
    }
    ensure!(
        payload.len() == usize::from(command.command[0]),
        "源写入报文长度不匹配"
    );
    Ok(ReadCommand {
        name: command.read_kind,
        method: command.method.clone(),
        command: command.command,
        payload,
        min_response_bytes: command.min_response_bytes,
    })
}

#[derive(Debug, Serialize)]
pub struct DeviceWriteResult {
    pub requested: DeviceWriteSetting,
    pub previous: DeviceReadValue,
    pub observed: DeviceReadValue,
    pub changed: bool,
    /// A transport send alone cannot construct this result.
    pub verified: bool,
}

/// Read -> optional setter/ack -> readback on one retained route/lock.
/// DPI and idle readback follow the original callers. The initial idle read
/// and skipping an unchanged idle setter are application policy; the source
/// idle task sends its setter before readback. Polling readback is also an
/// application confirmation policy, distinguished in the source evidence.
pub fn apply(
    device: &dyn FeatureTransport,
    read_cap: &DeviceReadCapability,
    setting: &DeviceWriteSetting,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DeviceWriteResult> {
    let cap = capability(read_cap.product_id).context("产品没有源核实的设备写入能力")?;
    ensure!(
        cap.source_class == read_cap.source_class,
        "读写源设备类不一致"
    );
    let setter = prepare(cap, setting)?;
    let getter = read_cap
        .queries
        .iter()
        .find(|command| command.name == setting.read_kind())
        .context("写入设置缺少源核实的回读查询")?;
    validate()?;
    let previous =
        device_query::read_device(device, read_cap, getter, next_transaction()?, &validate)
            .context("写入前读取失败，尚未发送设置")?;
    if setting.matches(&previous) {
        validate()?;
        return Ok(DeviceWriteResult {
            requested: setting.clone(),
            observed: previous.clone(),
            previous,
            changed: false,
            verified: true,
        });
    }
    validate()?;
    device_query::read_device(device, read_cap, &setter, next_transaction()?, &validate)
        .context("未能确认写入应答；设备可能已经接受设置，请先重新读取")?;
    validate().context("写入应答后设备身份或期限变化，未确认最终设置")?;
    let observed =
        device_query::read_device(device, read_cap, getter, next_transaction()?, &validate)
            .context("已收到写入应答，但重新读取失败，未确认最终设置")?;
    ensure!(
        setting.matches(&observed),
        "设备回读与请求设置不符，写入结果未确认"
    );
    validate().context("回读后设备身份或期限变化，写入结果未确认")?;
    Ok(DeviceWriteResult {
        requested: setting.clone(),
        previous,
        observed,
        changed: true,
        verified: true,
    })
}

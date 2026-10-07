//! Source-generated shared device query capabilities and session observations.
//! No local profile, route default or registry history supplies these values.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeviceReadKind {
    Firmware,
    Battery,
    Charging,
    Polling,
    Dpi,
}

impl DeviceReadKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Firmware => "firmware",
            Self::Battery => "battery",
            Self::Charging => "charging",
            Self::Polling => "polling",
            Self::Dpi => "dpi",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ReadCommand {
    pub(crate) name: DeviceReadKind,
    pub(crate) method: String,
    pub(crate) command: [u8; 3],
    pub(crate) payload: Vec<u8>,
    pub(crate) min_response_bytes: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct DeviceReadCapability {
    pub(crate) product_id: u32,
    pub(crate) source_class: String,
    pub(crate) direct_pids: Vec<u32>,
    pub(crate) vendor_id: u16,
    pub(crate) claim_interface: u8,
    pub(crate) report_id: u8,
    pub(crate) report_bytes: usize,
    pub(crate) transaction_prefix: u8,
    pub(crate) transaction_modulus: u8,
    pub(crate) queries: Vec<ReadCommand>,
    pub(crate) polling_codes: BTreeMap<String, u8>,
    pub(crate) charging_codes: BTreeMap<String, u8>,
    pub(crate) max_retry_in: u8,
    pub(crate) max_retry_out: u8,
    pub(crate) sleep_between_out_ms: u64,
    pub(crate) sleep_between_out_in_ms: u64,
    pub(crate) sleep_between_in_ms: u64,
}

pub(crate) fn capability(product_id: u32) -> Option<&'static DeviceReadCapability> {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        products: Vec<DeviceReadCapability>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let catalog: Catalog = serde_json::from_str(include_str!(
                "../../assets/data/device-read-capabilities.json"
            ))
            .expect("source-derived device read capabilities");
            assert_eq!(catalog.schema_version, 1);
            catalog
        })
        .products
        .iter()
        .find(|capability| capability.product_id == product_id)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct DeviceReadTarget {
    pub(crate) product_id: u32,
    pub(crate) physical_product_id: u32,
    pub(crate) peer_product_id: Option<u32>,
    pub(crate) device_container_id: String,
    pub(crate) path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum DeviceReadValue {
    Firmware { version: String },
    Battery { percent: u8 },
    Charging { status: String },
    Polling { hz: u32 },
    Dpi { x: u16, y: u16 },
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct DeviceReadValues {
    pub(crate) firmware: Option<String>,
    pub(crate) battery_percent: Option<u8>,
    pub(crate) charging_status: Option<String>,
    pub(crate) polling_hz: Option<u32>,
    pub(crate) dpi: Option<(u16, u16)>,
    pub(crate) errors: BTreeMap<String, String>,
}

impl DeviceReadValues {
    pub(crate) fn insert(
        &mut self,
        expected: DeviceReadKind,
        value: DeviceReadValue,
    ) -> anyhow::Result<()> {
        match (expected, value) {
            (DeviceReadKind::Firmware, DeviceReadValue::Firmware { version }) => {
                self.firmware = Some(version)
            }
            (DeviceReadKind::Battery, DeviceReadValue::Battery { percent }) => {
                self.battery_percent = Some(percent)
            }
            (DeviceReadKind::Charging, DeviceReadValue::Charging { status }) => {
                self.charging_status = Some(status)
            }
            (DeviceReadKind::Polling, DeviceReadValue::Polling { hz }) => {
                self.polling_hz = Some(hz)
            }
            (DeviceReadKind::Dpi, DeviceReadValue::Dpi { x, y }) => self.dpi = Some((x, y)),
            _ => anyhow::bail!("设备查询响应类型不匹配"),
        }
        Ok(())
    }
}

pub(crate) fn query_report(
    cap: &DeviceReadCapability,
    command: &ReadCommand,
    transaction: u8,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        cap.report_bytes == 91
            && cap.report_id == 0
            && command.command[0] <= 80
            && command.payload.len() <= usize::from(command.command[0]),
        "源查询报文规格无效"
    );
    let mut report = vec![0; cap.report_bytes];
    report[0] = cap.report_id;
    report[2] = transaction;
    report[6..9].copy_from_slice(&command.command);
    report[9..9 + command.payload.len()].copy_from_slice(&command.payload);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    Ok(report)
}

pub(crate) enum ReadReply {
    Busy,
    Retry(String),
    Complete(DeviceReadValue),
}

pub(crate) fn decode_report(
    report: &[u8],
    cap: &DeviceReadCapability,
    command: &ReadCommand,
    transaction: u8,
) -> anyhow::Result<ReadReply> {
    ensure!(
        report.len() == cap.report_bytes && report.len() == 91 && report[0] == cap.report_id,
        "设备查询响应长度或ReportID不匹配"
    );
    let packet = &report[1..];
    if packet[1] != transaction || packet[6..8] != command.command[1..] {
        return Ok(ReadReply::Retry("设备查询事务或命令不匹配".into()));
    }
    match packet[0] {
        1 => return Ok(ReadReply::Busy),
        2 => {}
        0 | 3 | 4 => return Ok(ReadReply::Retry(format!("设备查询响应状态 {}", packet[0]))),
        state => anyhow::bail!("设备查询不受支持或返回未知状态 {state}"),
    }
    let length = usize::from(packet[5]);
    ensure!(
        length >= command.min_response_bytes && length <= 80,
        "设备查询响应字段截断"
    );
    let data = &packet[8..8 + length];
    if let Some(expected) = command.payload.first() {
        ensure!(
            data[0] == *expected,
            "设备查询返回了其他battery/profile/class的数据"
        );
    }
    let value = match command.name {
        DeviceReadKind::Firmware => DeviceReadValue::Firmware {
            version: format!("{}.{}.{}.{}", data[0], data[1], data[2], data[3]),
        },
        DeviceReadKind::Battery => DeviceReadValue::Battery {
            percent: (u16::from(data[1]) * 100 / 255) as u8,
        },
        DeviceReadKind::Charging => DeviceReadValue::Charging {
            status: cap
                .charging_codes
                .iter()
                .find(|(_, code)| **code == data[1])
                .map(|(name, _)| name.clone())
                .context("未知充电状态编码")?,
        },
        DeviceReadKind::Polling => {
            let name = cap
                .polling_codes
                .iter()
                .find(|(_, code)| **code == data[1])
                .map(|(name, _)| name)
                .context("未知回报率编码")?;
            let hz = name
                .strip_prefix("RATE_")
                .and_then(|name| name.strip_suffix("Hz"))
                .context("源回报率名称无效")?
                .parse()?;
            DeviceReadValue::Polling { hz }
        }
        DeviceReadKind::Dpi => {
            // The source getter decodes the raw unsigned fields, including 0.
            // Product UI edit limits do not constrain a hardware observation.
            let x = u16::from_be_bytes([data[1], data[2]]);
            let y = u16::from_be_bytes([data[3], data[4]]);
            DeviceReadValue::Dpi { x, y }
        }
    };
    Ok(ReadReply::Complete(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-only protocol fixtures. No native transport or hardware is used.
    fn reply(kind: DeviceReadKind, data: &[u8]) -> (Vec<u8>, &'static ReadCommand) {
        let cap = capability(182).unwrap();
        let command = cap.queries.iter().find(|item| item.name == kind).unwrap();
        let mut report = query_report(cap, command, 7).unwrap();
        report[1] = 2;
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(data);
        (report, command)
    }

    #[test]
    fn reads_only_source_response_fields_and_preserves_units() {
        let cap = capability(182).unwrap();
        for (raw, expected) in [(0, 0), (128, 50), (255, 100)] {
            let (report, command) = reply(DeviceReadKind::Battery, &[0, raw]);
            assert!(matches!(
                decode_report(&report, cap, command, 7).unwrap(),
                ReadReply::Complete(DeviceReadValue::Battery { percent }) if percent == expected
            ));
        }
        let (report, command) = reply(DeviceReadKind::Polling, &[1, 8]);
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Complete(DeviceReadValue::Polling { hz: 1000 })
        ));
        let (report, command) = reply(DeviceReadKind::Dpi, &[0, 3, 32, 6, 64, 0, 0]);
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Complete(DeviceReadValue::Dpi { x: 800, y: 1600 })
        ));
        for (x, y) in [(800u16, 0u16), (0, 0), (65535, 65535)] {
            let xb = x.to_be_bytes();
            let yb = y.to_be_bytes();
            let (report, command) =
                reply(DeviceReadKind::Dpi, &[0, xb[0], xb[1], yb[0], yb[1], 0, 0]);
            assert!(matches!(
                decode_report(&report, cap, command, 7).unwrap(),
                ReadReply::Complete(DeviceReadValue::Dpi { x: actual_x, y: actual_y })
                    if (actual_x, actual_y) == (x, y)
            ));
        }
        let (report, command) = reply(DeviceReadKind::Firmware, &[2, 1, 3, 0]);
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Complete(DeviceReadValue::Firmware { version }) if version == "2.1.3.0"
        ));
    }

    #[test]
    fn rejects_unknown_codes_wrong_scope_and_partial_or_stale_reports() {
        let cap = capability(182).unwrap();
        for (kind, data) in [
            (DeviceReadKind::Polling, vec![1, 3]),
            (DeviceReadKind::Charging, vec![0, 99]),
            (DeviceReadKind::Battery, vec![1, 255]),
            (DeviceReadKind::Dpi, vec![1, 3, 32, 3, 32, 0, 0]),
            (DeviceReadKind::Firmware, vec![1, 2]),
        ] {
            let (report, command) = reply(kind, &data);
            assert!(decode_report(&report, cap, command, 7).is_err());
        }
        let (mut report, command) = reply(DeviceReadKind::Battery, &[0, 255]);
        assert!(matches!(
            decode_report(&report, cap, command, 8).unwrap(),
            ReadReply::Retry(_)
        ));
        report[8] ^= 1;
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Retry(_)
        ));
        assert!(decode_report(&report[..90], cap, command, 7).is_err());
    }

    #[test]
    fn query_header_payload_and_checksum_use_mouse_transaction_namespace() {
        let cap = capability(182).unwrap();
        let command = cap
            .queries
            .iter()
            .find(|item| item.name == DeviceReadKind::Dpi)
            .unwrap();
        let report = query_report(cap, command, 7).unwrap();
        assert_eq!(&report[..10], &[0, 0, 7, 0, 0, 0, 7, 4, 133, 0]);
        assert_eq!(report[89], 7 ^ 4 ^ 133);
        assert_eq!(cap.transaction_prefix, 0);
    }
}

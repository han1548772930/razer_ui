//! Shared current rzDevice25 query framing and V2 status decoding.
//! Product selection and native transport are deliberately outside this module.
use super::receiver_capabilities::{ReceiverCapability, ReceiverProtocol};
use anyhow::{bail, ensure};

const PACKET_BYTES: usize = 90;

pub(crate) fn query_report(
    capability: &ReceiverCapability,
    transaction: u8,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        capability.protocol == ReceiverProtocol::RazerDevice25WirelessStatusV2
            && capability.report_bytes == PACKET_BYTES + 1
            && capability.report_id == 0
            && capability.command == [80, 0, 191],
        "接收器能力与已核验协议不匹配"
    );
    let mut report = vec![0u8; capability.report_bytes];
    // The host prepends Windows ReportID to the middleware's 90-byte packet.
    report[0] = capability.report_id;
    report[2] = transaction;
    report[6..9].copy_from_slice(&capability.command);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    Ok(report)
}

#[derive(Debug, PartialEq)]
pub(crate) enum Reply {
    Complete(Vec<(u16, u8)>),
    Busy,
    Retry(&'static str),
}

pub(crate) fn decode_report(
    report: &[u8],
    capability: &ReceiverCapability,
    transaction: u8,
) -> anyhow::Result<Reply> {
    ensure!(
        report.len() == capability.report_bytes && report.len() == PACKET_BYTES + 1,
        "Feature 返回长度 {}，期望 {}",
        report.len(),
        capability.report_bytes
    );
    ensure!(report[0] == capability.report_id, "Feature ReportID 不匹配");
    let packet = &report[1..];
    if packet[1] != transaction {
        return Ok(Reply::Retry("事务 ID 不匹配"));
    }
    if packet[6..8] != capability.command[1..3] {
        return Ok(Reply::Retry("查询命令不匹配"));
    }
    match packet[0] {
        0 => return Ok(Reply::Retry("设备尚未处理命令")),
        1 => return Ok(Reply::Busy),
        2 => {}
        3 => return Ok(Reply::Retry("设备查询失败")),
        4 => return Ok(Reply::Retry("设备查询超时")),
        5 => bail!("设备不支持无线连接状态 V2 查询"),
        state => bail!("未知协议响应状态 {state}"),
    }
    let length = usize::from(packet[5]);
    ensure!((1..=80).contains(&length), "连接状态数据长度无效：{length}");
    let data = &packet[8..8 + length];
    let count = usize::from(data[0]);
    ensure!(
        1 + count * 3 <= data.len(),
        "连接状态记录截断：count={count}，数据长度={length}"
    );
    // Preserve firmware PIDs and raw status values. Product/catalog mapping and
    // readiness are separate observations; a malformed reply is never empty.
    Ok(Reply::Complete(
        data[1..1 + count * 3]
            .chunks_exact(3)
            .map(|triple| (u16::from_be_bytes([triple[1], triple[2]]), triple[0]))
            .collect(),
    ))
}

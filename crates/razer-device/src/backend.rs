//! Host-neutral HID contracts; native implementations depend on this module.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Descriptor-observed byte lengths, including the API's Report ID byte.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ReportLengths {
    pub input: BTreeMap<u8, usize>,
    pub output: BTreeMap<u8, usize>,
    pub feature: BTreeMap<u8, usize>,
}

/// Identifies a logical HID collection, not a physical product or receiver peer.
/// Preserve raw path bytes and usage pair so distinct collections stay distinct.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HidNode {
    pub path: Vec<u8>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
    pub release_number: u16,
    pub serial_number: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
}

pub trait HidBackend {
    fn enumerate(&mut self) -> anyhow::Result<Vec<HidNode>>;
    /// Only opens a uniquely re-observed collection. Never opens the first VID/PID.
    fn open(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn FeatureTransport>>;
}

pub trait FeatureTransport {
    /// Success means the backend checked a complete send; it is not a device
    /// acknowledgement. The protocol layer must validate the subsequent reply.
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()>;
    /// Actual byte count, including the Report ID. No guessed/padded response.
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize>;
    /// Output reports and control Input reports are distinct from Feature and
    /// interrupt reads. Backends must implement the exact requested channel.
    fn write_output(&self, _report: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("此后端未实现 Output Report")
    }
    fn get_input(&self, _report: &mut [u8]) -> anyhow::Result<usize> {
        anyhow::bail!("此后端未实现控制 Input Report")
    }
    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        anyhow::bail!("此后端没有 descriptor 观察的报告长度")
    }
    fn metadata(&self) -> Value;
}

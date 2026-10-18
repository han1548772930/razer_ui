//! Host-neutral HID contracts; native implementations depend on this module.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Descriptor-observed byte lengths, including the API's Report ID byte.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
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

pub trait InterruptTransport {
    fn read(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize>;
    fn report_lengths(&self) -> ReportLengths;
}

pub trait HidBackend {
    fn open_interrupt(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn InterruptTransport>>;
    fn enumerate(&mut self) -> anyhow::Result<Vec<HidNode>>;
    /// Only opens a uniquely re-observed collection. Never opens the first VID/PID.
    fn open(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn FeatureTransport>>;
}

pub trait FeatureTransport {
    /// The collection re-observed when this handle was opened. None carries no
    /// hardware identity; simulated transports may supply an explicit test node.
    fn hid_node(&self) -> Option<&HidNode> {
        None
    }

    /// Success means the backend checked a complete send; it is not a device
    /// acknowledgement. The protocol layer must validate the subsequent reply.
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()>;
    /// Source HID.node sends the complete collection buffer without requiring
    /// its prefix to be one of the descriptor's advertised IDs.
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.send_feature(report)
    }
    /// Actual byte count, including the Report ID. No guessed/padded response.
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize>;
    /// Collection-sized GET_REPORT used by a source host which selects an ID
    /// independently of the descriptor's advertised IDs. The native adapter
    /// still verifies the observed collection maximum and performs a real GET.
    /// Protocol callers must opt in; ordinary reads retain exact-ID checking.
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.get_feature(report)
    }
    /// Output reports and control Input reports are distinct from Feature and
    /// interrupt reads. Backends must implement the exact requested channel.
    fn write_output(&self, _report: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("此后端未实现 Output Report")
    }
    /// Opt-in source-host Output write. Preserve the supplied bytes, padding
    /// only to the observed collection minimum, and return the native count.
    /// Protocols with a source-specific count predicate must check it themselves;
    /// a complete-send adapter cannot supply a fabricated count for this channel.
    fn write_output_counted(&self, _report: &[u8]) -> anyhow::Result<usize> {
        anyhow::bail!("此后端未实现带实际字节数的 Output Report")
    }
    fn get_input(&self, _report: &mut [u8]) -> anyhow::Result<usize> {
        anyhow::bail!("此后端未实现控制 Input Report")
    }
    /// Interrupt IN bytes, distinct from a control GET_REPORT. Numbered reports
    /// retain their actual ID; unnumbered reports have no synthetic zero prefix.
    /// Returns zero on timeout, never a fabricated empty hardware event. A finite
    /// timeout lets the caller check cancellation and retained device identity.
    fn read_interrupt(&self, _report: &mut [u8], _timeout_ms: u32) -> anyhow::Result<usize> {
        anyhow::bail!("此后端未实现 Interrupt Input Report")
    }
    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        anyhow::bail!("此后端没有 descriptor 观察的报告长度")
    }
    fn metadata(&self) -> Value;
}

//! Host-neutral HID contracts; native implementations depend on this module.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Identifies a logical HID collection, not a physical product or receiver peer.
/// Preserve raw path bytes and usage pair (OpenLogi's macOS collection rule).
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
    fn metadata(&self) -> Value;
}

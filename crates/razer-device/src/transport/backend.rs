//! Host-neutral HID contracts; native implementations depend on this module.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "exchange_lease.rs"]
mod exchange_lease;
pub(crate) use exchange_lease::finalization_failure;
pub use exchange_lease::{
    ExchangeFinalizationFailure, ExchangeLease, ExchangeOptions, ExchangeTiming,
    ExchangeValidation, Protocol25Topology, SourceCommandScope, checked_exchange,
    checked_exchange_observing_send, checked_exchange_owned, exchange_finalization_failed,
    finish_exchange,
};

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

    /// Interrupt owners use the same caller-only cancellation boundary while
    /// their native initialization remains retained until its terminal return.
    fn open_interrupt_validated(
        &mut self,
        node: &HidNode,
        validate: ExchangeValidation<'_>,
    ) -> anyhow::Result<Box<dyn InterruptTransport>> {
        validate()?;
        let result = self.open_interrupt(node);
        match (result, validate()) {
            (Ok(transport), Ok(())) => Ok(transport),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(validation)) => Err(error.context(format!(
                "HID interrupt open validation also failed: {validation:#}"
            ))),
        }
    }
    fn enumerate(&mut self) -> anyhow::Result<Vec<HidNode>>;
    /// Only opens a uniquely re-observed collection. Never opens the first VID/PID.
    fn open(&mut self, node: &HidNode) -> anyhow::Result<Box<dyn FeatureTransport>>;

    /// Validation stays on the caller's thread. A pooled backend also checks
    /// it while waiting for a native owner, whose actual I/O may outlive cancellation.
    fn open_validated(
        &mut self,
        node: &HidNode,
        validate: ExchangeValidation<'_>,
    ) -> anyhow::Result<Box<dyn FeatureTransport>> {
        validate()?;
        let result = self.open(node);
        match (result, validate()) {
            (Ok(transport), Ok(())) => Ok(transport),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(validation)) => {
                Err(error.context(format!("HID open validation also failed: {validation:#}")))
            }
        }
    }
}

pub trait FeatureTransport {
    /// Hold a complete OUT/GET/retry operation. Independent transports already
    /// own an exclusive handle; broker proxies override this with their queue.
    fn begin_exchange(
        &self,
        options: ExchangeOptions,
    ) -> anyhow::Result<Box<dyn ExchangeLease + '_>> {
        exchange_lease::already_exclusive(self, options)
    }

    /// Keep the caller's identity/cancellation validation active while waiting
    /// for admission and across every packet, retry and explicit release.
    fn begin_exchange_validated<'a>(
        &'a self,
        options: ExchangeOptions,
        validate: ExchangeValidation<'a>,
    ) -> anyhow::Result<Box<dyn ExchangeLease + 'a>> {
        validate()?;
        let inner = self.begin_exchange(options)?;
        Ok(checked_exchange_owned(
            inner,
            validate,
            std::rc::Rc::new(|| Ok(())),
            None,
        ))
    }

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

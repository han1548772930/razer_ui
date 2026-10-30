//! Host-neutral HID contracts; native implementations depend on this module.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "native_io.rs"]
mod native_io;
pub(crate) use native_io::complete_native_io;
pub use native_io::{
    NativeIoOutcome, checked_native_io, checked_native_read, complete_native_read,
    native_io_attempted, native_io_failure, native_io_outcome, native_read_bytes,
    require_complete_send,
};

#[path = "feature_batch.rs"]
mod feature_batch;
pub use feature_batch::{
    FeatureBatch, FeatureBatchFailure, FeatureBatchOperation, FeatureBatchProgress,
    FeatureBatchReceipt, FeatureBatchReportAttempt, FeatureBatchReportReturn,
    checked_feature_batch, execute_feature_batch, observe_feature_batch,
};

#[path = "exchange_lease.rs"]
mod exchange_lease;
pub(crate) use exchange_lease::finalization_failure;
pub use exchange_lease::{
    ExchangeFinalizationFailure, ExchangeLease, ExchangeOptions, ExchangeTiming,
    ExchangeValidation, Protocol25Topology, SourceCommandScope, checked_exchange,
    checked_exchange_observing_send, checked_exchange_owned, exchange_finalization_failed,
    exchange_release_completed, finish_exchange, reject_exchange_release,
    validate_exchange_release,
};

/// Descriptor-observed byte lengths, including the API's Report ID byte.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ReportLengths {
    pub input: BTreeMap<u8, usize>,
    pub output: BTreeMap<u8, usize>,
    pub feature: BTreeMap<u8, usize>,
}

impl ReportLengths {
    /// Ordinary Output writes require an observed ID and either its exact
    /// extent or the collection maximum, including the API's Report ID byte.
    pub fn accepts_output_report(&self, report: &[u8]) -> bool {
        report
            .first()
            .is_some_and(|id| accepts_io_report_length(&self.output, *id, report.len()))
    }

    /// Input APIs may use the collection maximum while returning an observed
    /// report's shorter extent. The supplied count includes the Report ID byte.
    pub fn accepts_input_report_length(&self, report_id: u8, byte_count: usize) -> bool {
        accepts_io_report_length(&self.input, report_id, byte_count)
    }
}

fn accepts_io_report_length(lengths: &BTreeMap<u8, usize>, id: u8, byte_count: usize) -> bool {
    lengths.get(&id).is_some_and(|observed| {
        byte_count == *observed || lengths.values().max().copied() == Some(byte_count)
    })
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
            validate.clone(),
            validate,
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
    /// Expose the transport's normal return count independently of a protocol
    /// consumer's acceptance predicate. The legacy method's success contract
    /// already guarantees the complete requested extent.
    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.send_feature(report).map(|()| report.len())
    }
    /// Source HID.node sends the complete collection buffer without requiring
    /// its prefix to be one of the descriptor's advertised IDs.
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.send_feature(report)
    }
    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        self.send_feature_at_collection_max(report)
            .map(|()| report.len())
    }
    /// Execute one source batch to its terminal return, including normal zero.
    /// A queued backend moves this entire operation to its retained native owner.
    fn send_feature_batch(&self, batch: &FeatureBatch) -> anyhow::Result<FeatureBatchReceipt> {
        execute_feature_batch(
            batch,
            |report| self.send_feature_at_collection_max_counted(report),
            |milliseconds| {
                std::thread::sleep(std::time::Duration::from_millis(u64::from(milliseconds)));
                Ok(())
            },
        )
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

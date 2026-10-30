//! Identity-checked current local controller lighting Feature operations.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{
        FeatureTransport, HidNode, checked_native_io, checked_native_read, require_complete_send,
    },
    controller_lighting::{LightingOperation, LightingProtocol, capability},
};
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[path = "controller_lighting_owner.rs"]
mod owner;
pub(super) use owner::Controller;

pub(super) fn execute(
    node: &HidNode,
    product_id: u32,
    operation: &LightingOperation,
) -> anyhow::Result<Value> {
    operation.validate_product(product_id)?;
    let source =
        capability(product_id).context("No mounted current controller lighting capability")?;
    ensure!(
        node.vendor_id == 5426
            && source.accepts(u32::from(node.product_id))
            && !node.path.is_empty(),
        "Controller lighting collection belongs to another product"
    );
    #[cfg(windows)]
    let proof = if node.interface_number == -1 {
        Some(super::native::controller_feature_node_proof(
            node,
            u8::try_from(source.claim_interface())?,
        )?)
    } else {
        None
    };
    ensure!(
        node.interface_number == source.claim_interface()
            || cfg!(windows) && node.interface_number == -1,
        "Controller lighting claim interface differs from current source"
    );
    let deadline = Instant::now() + Duration::from_secs(90);
    let validate = || -> anyhow::Result<()> {
        ensure!(
            Instant::now() < deadline,
            "Controller lighting operation timed out"
        );
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Controller lighting collection changed or is ambiguous"
        );
        #[cfg(windows)]
        if let Some(proof) = &proof {
            ensure!(
                super::native::controller_feature_node_proof(
                    node,
                    u8::try_from(source.claim_interface())?,
                )? == *proof,
                "Controller lighting Windows instance changed"
            );
        }
        Ok(())
    };
    validate()?;
    let device = with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
    ensure!(
        device.report_lengths()?.feature.get(&source.report_id()) == Some(&91),
        "Controller descriptor does not expose the source lighting Feature report"
    );
    let checked = CheckedTransport {
        inner: device.as_ref(),
        validate: &validate,
    };
    let wait = |duration| {
        validate()?;
        std::thread::sleep(duration);
        validate()
    };
    let result = LightingProtocol::new(&checked, source, wait).execute(operation)?;
    validate()?;
    Ok(
        json!({"node": node, "product_id": product_id, "operation": operation,
        "device_write_acknowledged": result.acknowledged_writes > 0,
        "device_read_observed": result.effect.is_some() || result.brightness.is_some(),
        "lighting_result": result, "transport_metadata": device.metadata()}),
    )
}

struct CheckedTransport<'a> {
    inner: &'a dyn FeatureTransport,
    validate: &'a dyn Fn() -> anyhow::Result<()>,
}
impl FeatureTransport for CheckedTransport<'_> {
    fn begin_exchange(
        &self,
        options: razer_device::backend::ExchangeOptions,
    ) -> anyhow::Result<Box<dyn razer_device::backend::ExchangeLease + '_>> {
        (self.validate)()?;
        Ok(razer_device::backend::checked_exchange(
            self.inner
                .begin_exchange_validated(options, std::rc::Rc::new(self.validate))?,
            self.validate,
            self.validate,
            None,
        ))
    }
    fn begin_exchange_validated<'a>(
        &'a self,
        options: razer_device::backend::ExchangeOptions,
        validate: razer_device::backend::ExchangeValidation<'a>,
    ) -> anyhow::Result<Box<dyn razer_device::backend::ExchangeLease + 'a>> {
        let own = self.validate;
        let combined = std::rc::Rc::new(move || {
            own()?;
            validate()
        });
        self.inner.begin_exchange_validated(options, combined)
    }
    fn hid_node(&self) -> Option<&HidNode> {
        self.inner.hid_node()
    }
    fn send_feature_batch(
        &self,
        batch: &razer_device::backend::FeatureBatch,
    ) -> anyhow::Result<razer_device::backend::FeatureBatchReceipt> {
        razer_device::backend::checked_feature_batch(
            self.validate,
            || self.inner.send_feature_batch(batch),
            self.validate,
        )
    }
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        let count = self.send_feature_counted(report)?;
        require_complete_send(count, report.len())
    }
    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            self.validate,
            || self.inner.send_feature_counted(report),
            self.validate,
        )
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            self.validate,
            report,
            |report| self.inner.get_feature(report),
            self.validate,
        )
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        let count = self.send_feature_at_collection_max_counted(report)?;
        require_complete_send(count, report.len())
    }
    fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        checked_native_io(
            self.validate,
            || self.inner.send_feature_at_collection_max_counted(report),
            self.validate,
        )
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        checked_native_read(
            self.validate,
            report,
            |report| self.inner.get_feature_at_collection_max(report),
            self.validate,
        )
    }
    fn metadata(&self) -> Value {
        self.inner.metadata()
    }
}

//! Exclusive, identity-checked current controller Power Feature transfers.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{FeatureTransport, HidNode},
    controller_sleep::{SleepOperation, SleepProtocol, capability},
};
use razer_hid::with_backend;
use razer_ipc::controller_sleep::SleepResponse;
use serde_json::Value;
use std::time::{Duration, Instant};

pub(super) fn execute(
    node: &HidNode,
    product_id: u32,
    operation: &SleepOperation,
) -> anyhow::Result<Value> {
    operation.validate()?;
    with_transport(
        node,
        product_id,
        Duration::from_secs(90),
        |checked, wait| {
            let source =
                capability(product_id).context("No current controller sleep capability")?;
            let result = SleepProtocol::new(checked, source, wait).execute(operation)?;
            serde_json::to_value(SleepResponse::new(
                node.clone(),
                product_id,
                operation.clone(),
                result,
                checked.metadata(),
            )?)
            .map_err(Into::into)
        },
    )
}

pub(super) fn with_transport<R>(
    node: &HidNode,
    product_id: u32,
    timeout: Duration,
    operation: impl FnOnce(
        &dyn FeatureTransport,
        &mut dyn FnMut(Duration) -> anyhow::Result<()>,
    ) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let source = capability(product_id).context("No current controller sleep capability")?;
    ensure!(
        node.vendor_id == 5426
            && source.accepts(u32::from(node.product_id))
            && !node.path.is_empty(),
        "Controller sleep collection belongs to another product"
    );
    #[cfg(windows)]
    let proof = if node.interface_number == -1 {
        Some(super::native::controller_calibration_node_proof(node)?)
    } else {
        None
    };
    ensure!(
        node.interface_number == 1 || cfg!(windows) && node.interface_number == -1,
        "Controller sleep claim interface differs from current source"
    );
    let deadline = Instant::now() + timeout;
    let validate = || -> anyhow::Result<()> {
        ensure!(
            Instant::now() < deadline,
            "Controller sleep operation timed out"
        );
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Controller sleep collection changed or is ambiguous"
        );
        #[cfg(windows)]
        if let Some(proof) = &proof {
            ensure!(
                super::native::controller_calibration_node_proof(node)? == *proof,
                "Controller sleep Windows instance changed"
            );
        }
        Ok(())
    };
    validate()?;
    let device = with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
    ensure!(
        device.report_lengths()?.feature.get(&source.report_id()) == Some(&91),
        "Controller descriptor does not expose the source Power Feature report"
    );
    let checked = CheckedTransport {
        inner: device.as_ref(),
        validate: &validate,
    };
    let mut wait = |duration| {
        validate()?;
        std::thread::sleep(duration);
        validate()
    };
    let result = operation(&checked, &mut wait)?;
    validate()?;
    Ok(result)
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
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        (self.validate)()?;
        self.inner.send_feature(report)?;
        (self.validate)()
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        (self.validate)()?;
        let count = self.inner.get_feature(report)?;
        (self.validate)()?;
        Ok(count)
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        (self.validate)()?;
        let count = self.inner.get_feature_at_collection_max(report)?;
        (self.validate)()?;
        Ok(count)
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        (self.validate)()?;
        self.inner.send_feature_at_collection_max(report)?;
        (self.validate)()
    }
    fn metadata(&self) -> Value {
        self.inner.metadata()
    }
}

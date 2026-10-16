//! Exclusive current controller Feature exchanges. The caller's IPC scope
//! guard cancels its owned worker; this adapter also enforces a finite deadline
//! and re-observes collection identity before every transfer and retry.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{FeatureTransport, HidBackend, HidNode},
    controller_triggers::{TriggerOperation, TriggerProtocol, capability},
};
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) fn execute(
    node: &HidNode,
    product_id: u32,
    operation: &TriggerOperation,
) -> anyhow::Result<Value> {
    operation.validate()?;
    let source = capability(product_id).context("No current controller trigger capability")?;
    ensure!(
        node.vendor_id == 5426
            && source.accepts(u32::from(node.product_id))
            && !node.path.is_empty(),
        "Controller trigger collection belongs to another product"
    );
    #[cfg(windows)]
    let proof = if node.interface_number == -1 {
        Some(super::native::controller_calibration_node_proof(node)?)
    } else {
        None
    };
    ensure!(
        node.interface_number == 1 || cfg!(windows) && node.interface_number == -1,
        "Controller trigger claim interface differs from current source"
    );
    let deadline = Instant::now() + Duration::from_secs(90);
    let validate = || -> anyhow::Result<()> {
        ensure!(
            Instant::now() < deadline,
            "Controller trigger operation timed out"
        );
        let nodes = with_backend(|backend| backend.enumerate())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Controller trigger collection changed or is ambiguous"
        );
        #[cfg(windows)]
        if let Some(proof) = &proof {
            ensure!(
                super::native::controller_calibration_node_proof(node)? == *proof,
                "Controller trigger Windows instance changed"
            );
        }
        Ok(())
    };
    validate()?;
    let device = with_backend(|backend| backend.open(node))?;
    ensure!(
        device.report_lengths()?.feature.get(&source.report_id()) == Some(&91),
        "Controller descriptor does not expose the source Feature report"
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
    let profile = TriggerProtocol::new(&checked, source, wait).execute(operation)?;
    validate()?;
    Ok(
        json!({"node":node,"product_id":product_id,"operation":operation,
        "profile":profile,"device_write_acknowledged":!operation.is_read(),
        "device_read_observed":operation.is_read(),"transport_metadata":device.metadata()}),
    )
}

struct CheckedTransport<'a> {
    inner: &'a dyn FeatureTransport,
    validate: &'a dyn Fn() -> anyhow::Result<()>,
}
impl FeatureTransport for CheckedTransport<'_> {
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

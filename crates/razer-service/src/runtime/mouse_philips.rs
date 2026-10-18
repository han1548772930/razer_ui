//! Collection handle and source factory for PID 70 calibration.
use super::*;
use razer_device::mouse_philips::{self as philips, Setting, SubmissionReport};
use std::sync::atomic::{AtomicBool, Ordering};
impl PortableRuntime {
    pub(in crate::runtime) fn mouse_philips(
        &mut self,
        node: &HidNode,
        product_id: u32,
        setting: Setting,
        canceled: &AtomicBool,
    ) -> anyhow::Result<SubmissionReport> {
        let cap = razer_device::mouse_legacy_polling::read_capability();
        philips::validate_transport(cap)?;
        ensure!(
            product_id == philips::PRODUCT_ID
                && node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == product_id
                && node.interface_number == i32::from(cap.claim_interface_for(product_id)?),
            "Philips calibration collection differs from its source factory"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                !canceled.load(Ordering::Acquire),
                "Philips calibration canceled"
            );
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Philips calibration deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device = with_backend(|backend| backend.open(node))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Philips calibration Feature length differs"
        );
        validate()?;
        let report = philips::submit(
            device.as_ref(),
            cap,
            setting,
            || {
                self.transaction(
                    node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )
            },
            &validate,
        );
        drop(device);
        Ok(report)
    }
}

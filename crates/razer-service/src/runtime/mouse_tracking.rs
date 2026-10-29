//! Collection adapter owns the actual handle, process lock and identity checks.
use super::*;
use razer_device::mouse_tracking::{self as tracking, SubmissionReport, TrackingSetting};
use std::sync::atomic::{AtomicBool, Ordering};

impl PortableRuntime {
    pub(in crate::runtime) fn mouse_tracking(
        &mut self,
        node: &HidNode,
        product_id: u32,
        peer: Option<u32>,
        setting: TrackingSetting,
        canceled: &AtomicBool,
    ) -> anyhow::Result<SubmissionReport> {
        tracking::prepare(
            tracking::capability(product_id).context("Orca Plus source is absent")?,
            setting,
        )?;
        let source = razer_device::mouse_current_dpi::capability(product_id)
            .context("Tracking physical factory is absent")?;
        let physical = u32::from(node.product_id);
        let session = peer
            .map(|peer| source.receiver_session(physical, peer))
            .transpose()?;
        let direct_cap;
        let cap = if let Some(session) = &session {
            session.transport()
        } else {
            ensure!(
                source.is_physical_branch(physical, false),
                "Tracking requires its source wired or bound radio sender"
            );
            direct_cap = source.transport().for_physical_product(physical)?;
            &direct_cap
        };
        ensure!(
            node.vendor_id == cap.vendor_id
                && node.interface_number == i32::from(cap.claim_interface_for(physical)?),
            "Tracking collection interface differs"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                !canceled.load(Ordering::Acquire),
                "Tracking operation canceled"
            );
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Tracking operation deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Tracking Feature Report size differs"
        );
        validate()?;
        let report = tracking::submit(
            device.as_ref(),
            cap,
            setting,
            session.as_ref(),
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
        // The owned transport and its lock are dropped before the controller
        // publishes terminal. The setter-only source has no rollback sequence.
        drop(device);
        Ok(report)
    }
}

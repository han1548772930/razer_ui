use super::*;
use razer_device::{
    mouse_current_dpi as factory,
    mouse_scroll::{MouseScroll, Submission},
};

impl PortableRuntime {
    pub(super) fn mouse_scroll(
        &mut self,
        node: &HidNode,
        product_id: u32,
        submission: &Submission,
    ) -> anyhow::Result<Value> {
        submission.validate(product_id)?;
        let source = factory::capability(product_id).context("Current scroll factory is absent")?;
        let physical = u32::from(node.product_id);
        let cap = source.transport().for_physical_product(physical)?;
        ensure!(
            node.vendor_id == cap.vendor_id
                && cap.direct_pids.contains(&physical)
                && node.interface_number == i32::from(cap.claim_interface_for(physical)?),
            "Scroll collection differs from current source"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(physical) else {
            bail!("Scroll physical identity is ambiguous");
        };
        ensure!(
            identity.product_id == product_id
                && !identity.is_ble
                && source.is_physical_branch(physical, identity.is_dongle),
            "Scroll physical branch differs"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Scroll confirmation deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Scroll Feature Report size differs"
        );
        let mut next = || {
            self.transaction(
                node,
                &cap.source_class,
                cap.transaction_prefix,
                cap.transaction_modulus,
            )
        };
        if identity.is_dongle {
            factory::verify_wireless_online_for(product_id, device.as_ref(), next()?, &validate)?;
        }
        let report =
            MouseScroll::new(device.as_ref(), &cap)?.submit(submission, &mut next, &validate)?;
        if identity.is_dongle {
            factory::verify_wireless_online_for(product_id, device.as_ref(), next()?, &validate)?;
        }
        validate()?;
        Ok(
            json!({"node":node,"product_id":product_id,"report":report,"source_class":cap.source_class,
            "setting_scope":"source_scroll_profile","transport_metadata":device.metadata(),"identity_scope":"hid_collection"}),
        )
    }
}

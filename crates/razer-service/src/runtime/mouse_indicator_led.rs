use super::*;
use razer_device::mouse_indicator_led::{self as indicator, Change};
impl PortableRuntime {
    pub(super) fn mouse_indicator_led(
        &mut self,
        node: &HidNode,
        product_id: u32,
        change: &Change,
    ) -> anyhow::Result<Value> {
        let source = indicator::capability(product_id).context("Three-LED source is absent")?;
        let cap = source.transport(u32::from(node.product_id))?;
        ensure!(
            node.vendor_id == cap.vendor_id
                && node.interface_number == i32::from(cap.claim_interface),
            "Three-LED physical receiver interface differs"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
        else {
            bail!("Three-LED receiver identity is ambiguous");
        };
        ensure!(
            identity.product_id == product_id && !identity.is_ble,
            "Three-LED physical product differs from its current factory"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Receiver LED task deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.get(&0) == Some(&91),
            "Receiver LED Feature size differs"
        );
        let report = indicator::execute(
            product_id,
            device.as_ref(),
            &cap,
            change,
            || {
                self.transaction(
                    node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )
            },
            &validate,
        )?;
        validate()?;
        Ok(json!({"node":node,"product_id":product_id,"report":report,
            "setting_scope":"active_profile_receiver_indicator","source_class":cap.source_class,
            "transport_metadata":device.metadata(),"identity_scope":"hid_collection"}))
    }
}

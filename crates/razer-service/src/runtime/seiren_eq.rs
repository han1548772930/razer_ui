use super::*;
use razer_device::seiren_eq::{self as eq, Change};
impl PortableRuntime {
    pub(super) fn seiren_eq(
        &mut self,
        node: &HidNode,
        product_id: u32,
        change: &Change,
    ) -> anyhow::Result<Value> {
        let cap = eq::capability(product_id).context("Seiren EQ transport source is absent")?;
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == product_id
                && node.interface_number == i32::from(cap.claim_interface_for(product_id)?),
            "Seiren EQ HID collection differs"
        );
        eq::validate_change(product_id, change)?;
        ensure!(
            !change.requires_native(),
            "Marie native DLL audio operations require the Windows adapter"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(product_id) else {
            bail!("Seiren EQ physical identity is ambiguous")
        };
        ensure!(
            identity.product_id == product_id && !identity.is_ble && !identity.is_dongle,
            "Seiren EQ requires its observed wired product"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Seiren EQ task deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device = with_backend(|backend| backend.open(node))?;
        ensure!(
            device.report_lengths()?.feature.get(&0) == Some(&91),
            "Seiren EQ Feature Report size differs"
        );
        let next = || {
            self.transaction(
                node,
                &cap.source_class,
                cap.transaction_prefix,
                cap.transaction_modulus,
            )
        };
        let report = eq::execute(product_id, device.as_ref(), cap, change, next, &validate)?;
        validate()?;
        Ok(
            json!({"node":node,"product_id":product_id,"report":report,"setting_scope":"seiren_eq_profile",
            "source_class":cap.source_class,"transport_metadata":device.metadata(),"identity_scope":"hid_collection"}),
        )
    }
}

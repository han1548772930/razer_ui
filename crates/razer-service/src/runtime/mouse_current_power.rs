use super::*;
use razer_device::mouse_current_power::{self as power, Change};
impl PortableRuntime {
    pub(super) fn mouse_current_power(
        &mut self,
        node: &HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
        change: &Change,
    ) -> anyhow::Result<Value> {
        if let Some(peer) = peer_product_id {
            return self.receiver_setting(
                node,
                product_id,
                peer,
                super::super::receiver_settings::Setting::CurrentPower(change),
            );
        }
        let source = power::capability(product_id).context("Mouse power source is absent")?;
        let physical = u32::from(node.product_id);
        let cap = source.transport(physical, false)?;
        ensure!(
            node.vendor_id == cap.vendor_id
                && source.accepts(physical, None)
                && node.interface_number == i32::from(cap.claim_interface_for(physical)?),
            "Power collection differs from current source"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(physical) else {
            bail!("Power physical identity is ambiguous");
        };
        ensure!(
            identity.product_id == product_id && !identity.is_ble,
            "Power physical product differs"
        );
        power::validate_change(product_id, change)?;
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Power task deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Power Feature Report size differs"
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
            power::verify_wireless_online(device.as_ref(), &cap, next()?, &validate)?;
        }
        let report = power::execute(
            product_id,
            device.as_ref(),
            &cap,
            change,
            &mut next,
            &validate,
        )?;
        if identity.is_dongle {
            power::verify_wireless_online(device.as_ref(), &cap, next()?, &validate)?;
        }
        validate()?;
        Ok(
            json!({"node":node,"product_id":product_id,"report":report,"setting_scope":"basic_device",
            "source_class":cap.source_class,"transport_metadata":device.metadata(),"identity_scope":"hid_collection"}),
        )
    }
}

#[cfg(test)]
#[path = "mouse_current_power_tests.rs"]
mod tests;

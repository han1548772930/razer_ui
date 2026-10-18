//! HID handles and source transaction counters stay with the service owner.
use super::*;
use razer_device::mouse_dynamic::{self as dynamic, Setting};

impl PortableRuntime {
    pub(super) fn mouse_dynamic(
        &mut self,
        node: &HidNode,
        product_id: u32,
        change: Option<&Setting>,
    ) -> anyhow::Result<Value> {
        let source =
            dynamic::capability(product_id).context("Dynamic physical factory is absent")?;
        let cap = source
            .transport()
            .for_physical_product(u32::from(node.product_id))?;
        let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
        else {
            bail!("Dynamic physical identity is ambiguous");
        };
        ensure!(
            identity.product_id == product_id
                && !identity.is_ble
                && source.is_physical_branch(u32::from(node.product_id), identity.is_dongle)
                && node.vendor_id == cap.vendor_id
                && node.interface_number == i32::from(cap.claim_interface),
            "Dynamic HID collection differs from the current factory"
        );
        if let Some(change) = change {
            change.validate()?;
        }
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Dynamic device task deadline expired"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device = with_backend(|backend| backend.open(node))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Dynamic Feature report length differs"
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
            razer_device::mouse_current_dpi::verify_wireless_online_for(
                product_id,
                device.as_ref(),
                next()?,
                &validate,
            )?;
        }
        let report = dynamic::execute(
            device.as_ref(),
            &cap,
            change,
            &mut next,
            &validate,
            &mut |duration| {
                std::thread::sleep(duration);
                validate()
            },
        )?;
        if identity.is_dongle {
            razer_device::mouse_current_dpi::verify_wireless_online_for(
                product_id,
                device.as_ref(),
                next()?,
                &validate,
            )?;
        }
        validate()?;
        Ok(
            json!({"node":node,"product_id":product_id,"source_class":source.transport().source_class,
            "transport_source_class":cap.source_class,"report":report,"profile_scope":"dynamic_profile_1",
            "transport_metadata":device.metadata(),"identity_scope":"hid_collection"}),
        )
    }
}

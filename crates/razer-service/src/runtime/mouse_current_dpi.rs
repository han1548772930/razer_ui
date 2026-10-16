use super::*;
use razer_device::mouse_current_dpi::{self as dpi, DpiChange};

impl PortableRuntime {
    pub(super) fn mouse_current_dpi(
        &mut self,
        node: &HidNode,
        product_id: u32,
        change: Option<&DpiChange>,
    ) -> anyhow::Result<Value> {
        let source = dpi::capability(product_id)
            .context("Mouse DPI collection has no current source factory capability")?;
        let cap = source
            .transport()
            .for_physical_product(u32::from(node.product_id))?;
        ensure!(
            node.vendor_id == cap.vendor_id
                && cap.direct_pids.contains(&u32::from(node.product_id))
                && node.interface_number
                    == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
            "Current mouse DPI collection differs from its current source interface"
        );
        let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
        else {
            bail!("Current mouse DPI target has no unique current physical identity");
        };
        ensure!(
            identity.product_id == product_id
                && !identity.is_ble
                && source.is_physical_branch(u32::from(node.product_id), identity.is_dongle),
            "Current mouse DPI physical branch differs"
        );
        if let Some(change) = change {
            dpi::validate_change_for(product_id, change)?;
        }
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Current mouse DPI confirmation deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device = with_backend(|backend| backend.open(node))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Current mouse DPI Feature Report size differs"
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
            dpi::verify_wireless_online_for(product_id, device.as_ref(), next()?, &validate)?;
        }
        let report = dpi::execute_for(product_id, device.as_ref(), change, &mut next, &validate)?;
        if identity.is_dongle {
            dpi::verify_wireless_online_for(product_id, device.as_ref(), next()?, &validate)?;
        }
        validate()?;
        Ok(
            json!({"node":node,"product_id":product_id,"source_class":source.transport().source_class,
            "transport_source_class":cap.source_class,
            "report":report,"transport_metadata":device.metadata(),"profile_scope":"stage_1_active_0",
            "identity_scope":"hid_collection","elapsed_ms":started.elapsed().as_millis() as u64}),
        )
    }
}

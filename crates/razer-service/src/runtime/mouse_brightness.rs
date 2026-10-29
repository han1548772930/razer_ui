use super::*;
use razer_device::{
    mouse_brightness::{self as brightness, Submission},
    mouse_current_dpi as factory,
};

impl PortableRuntime {
    pub(super) fn mouse_brightness(
        &mut self,
        node: &HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
        submission: &Submission,
    ) -> anyhow::Result<Value> {
        submission.validate(product_id)?;
        if let Some(peer) = peer_product_id {
            return self.receiver_setting(
                node,
                product_id,
                peer,
                super::super::receiver_settings::Setting::MouseBrightness(submission),
            );
        }
        let source = factory::capability(product_id)
            .context("Current brightness physical factory is absent")?;
        let physical = u32::from(node.product_id);
        let cap = source.transport().for_physical_product(physical)?;
        let IdentityLookup::Unique(identity) = device_identity::lookup(physical) else {
            bail!("Brightness physical identity is ambiguous");
        };
        ensure!(
            node.vendor_id == cap.vendor_id
                && cap.direct_pids.contains(&physical)
                && node.interface_number == i32::from(cap.claim_interface_for(physical)?)
                && identity.product_id == product_id
                && !identity.is_ble
                && source.is_physical_branch(physical, identity.is_dongle),
            "Brightness physical branch differs from current source"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(60),
                "Brightness device task deadline expired"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Brightness Feature report length differs"
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
        let report = brightness::execute(device.as_ref(), &cap, submission, &mut next, &validate)?;
        if identity.is_dongle {
            factory::verify_wireless_online_for(product_id, device.as_ref(), next()?, &validate)?;
        }
        validate()?;
        Ok(
            json!({"node":node,"product_id":product_id,"report":report,"source_class":source.transport().source_class,"transport_source_class":cap.source_class,"setting_scope":"active_and_matching_obm_brightness","transport_metadata":device.metadata(),"identity_scope":"hid_collection"}),
        )
    }
}

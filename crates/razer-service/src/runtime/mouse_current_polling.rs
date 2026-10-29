use super::*;
use razer_device::mouse_current_polling as polling;

impl PortableRuntime {
    pub(super) fn mouse_current_polling(
        &mut self,
        node: &HidNode,
        product_id: u32,
        change: Option<&polling::Change>,
    ) -> anyhow::Result<Value> {
        ensure!(
            product_id == polling::PRODUCT_ID,
            "Polling logical product differs"
        );
        if let Some(change) = change {
            change.validate()?;
        }
        let cap = polling::transport(u32::from(node.product_id))?;
        let IdentityLookup::Unique(identity) = device_identity::lookup(u32::from(node.product_id))
        else {
            bail!("Polling target has no unique physical identity");
        };
        ensure!(
            identity.product_id == product_id
                && !identity.is_ble
                && node.vendor_id == cap.vendor_id
                && node.interface_number == i32::from(cap.claim_interface),
            "Polling collection differs from the current physical factory"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Polling task deadline expired"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device =
            with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(cap.report_bytes),
            "Polling Feature report length differs"
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
        let (report, reading) = if let Some(change) = change {
            (
                Some(polling::request(
                    device.as_ref(),
                    &cap,
                    change,
                    &mut next,
                    &validate,
                )?),
                None,
            )
        } else {
            (
                None,
                Some(polling::read(device.as_ref(), &cap, &mut next, &validate)?),
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
        validate()?;
        Ok(
            json!({"node":node,"product_id":product_id,"report":report,"reading":reading,
            "source_class":cap.source_class,"method":if change.is_some() { "setUSBHighSpeedPollingRate" } else { "getUSBHighSpeedPollingRate" },
            "profile_scope":"current_profile_1","obm_profiles_written":false,
            "transport_metadata":device.metadata(),"identity_scope":"hid_collection",
            "elapsed_ms":started.elapsed().as_millis() as u64}),
        )
    }
}

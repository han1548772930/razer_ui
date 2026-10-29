use super::*;
use razer_device::mouse_current_polling as polling;

pub(super) fn request(
    target: &DeviceReadTarget,
    change: Option<&polling::Change>,
) -> anyhow::Result<Value> {
    ensure!(
        target.product_id == polling::PRODUCT_ID && target.peer_product_id.is_none(),
        "Polling requires its wired mouse or ordinary receiver"
    );
    if let Some(change) = change {
        change.validate()?;
    }
    let cap = polling::transport(target.physical_product_id)?;
    let IdentityLookup::Unique(identity) = device_identity::lookup(target.physical_product_id)
    else {
        anyhow::bail!("Polling physical identity is ambiguous");
    };
    ensure!(
        identity.product_id == target.product_id && !identity.is_ble,
        "Polling physical product differs"
    );
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Polling task deadline expired"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Polling instance changed during communication"
        );
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    validate()?;
    ensure!(
        before["feature_report_bytes"] == cap.report_bytes,
        "Polling Feature report length differs"
    );
    let device =
        hid_transport::open_validated(&target.path, cap.report_bytes, std::rc::Rc::new(&validate))?;
    let mut next = || transaction(target, &cap);
    if identity.is_dongle {
        razer_device::mouse_current_dpi::verify_wireless_online_for(
            target.product_id,
            &device,
            next()?,
            &validate,
        )?;
    }
    let (report, reading) = if let Some(change) = change {
        (
            Some(polling::request(
                &device, &cap, change, &mut next, &validate,
            )?),
            None,
        )
    } else {
        (
            None,
            Some(polling::read(&device, &cap, &mut next, &validate)?),
        )
    };
    if identity.is_dongle {
        razer_device::mouse_current_dpi::verify_wireless_online_for(
            target.product_id,
            &device,
            next()?,
            &validate,
        )?;
    }
    validate()?;
    Ok(
        json!({"target":target,"report":report,"reading":reading,"source_class":cap.source_class,
        "method":if change.is_some() { "setUSBHighSpeedPollingRate" } else { "getUSBHighSpeedPollingRate" },"profile_scope":"current_profile_1",
        "obm_profiles_written":false,"transport_metadata":device.metadata(),
        "elapsed_ms":started.elapsed().as_millis() as u64}),
    )
}

use super::*;
use razer_device::mouse_current_dpi::{self as dpi, DpiChange};

pub(super) fn request(
    target: &DeviceReadTarget,
    change: Option<&DpiChange>,
) -> anyhow::Result<Value> {
    let source = dpi::capability(target.product_id)
        .context("Mouse DPI target has no current source factory capability")?;
    let cap = source
        .transport()
        .for_physical_product(target.physical_product_id)?;
    ensure!(
        cap.direct_pids.contains(&target.physical_product_id) && target.peer_product_id.is_none(),
        "Current mouse DPI target differs from the source's single-device adapter"
    );
    let IdentityLookup::Unique(identity) = device_identity::lookup(target.physical_product_id)
    else {
        anyhow::bail!("Current mouse DPI target has no unique physical identity");
    };
    ensure!(
        identity.product_id == target.product_id
            && !identity.is_ble
            && source.is_physical_branch(target.physical_product_id, identity.is_dongle),
        "Current mouse DPI physical branch differs"
    );
    if let Some(change) = change {
        dpi::validate_change_for(target.product_id, change)?;
    }
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Current mouse DPI confirmation deadline exceeded"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Current mouse DPI device instance changed during communication"
        );
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .context("Current mouse DPI Feature Report size is absent")?
        as usize;
    ensure!(
        feature_bytes == cap.report_bytes,
        "Current mouse DPI observed Feature Report size differs"
    );
    let device = hid_transport::open(&target.path, feature_bytes)?;
    let mut next = || transaction(target, &cap);
    if identity.is_dongle {
        dpi::verify_wireless_online_for(target.product_id, &device, next()?, &validate)?;
    }
    let report = dpi::execute_for(target.product_id, &device, change, &mut next, &validate)?;
    if identity.is_dongle {
        dpi::verify_wireless_online_for(target.product_id, &device, next()?, &validate)?;
    }
    validate()?;
    Ok(
        json!({"target":target,"source_class":source.transport().source_class,"report":report,
        "transport_source_class":cap.source_class,
        "transport_metadata":device.metadata(),"profile_scope":"stage_1_active_0",
        "elapsed_ms":started.elapsed().as_millis() as u64}),
    )
}

use super::*;
use razer_device::mouse_rotation::{self as rotation, Setting};
use std::time::{Duration, Instant};

pub(super) fn request(
    target: &DeviceReadTarget,
    change: Option<&Setting>,
) -> anyhow::Result<Value> {
    let source =
        rotation::capability(target.product_id).context("Rotation current factory is absent")?;
    let cap = source
        .transport()
        .for_physical_product(target.physical_product_id)?;
    ensure!(
        target.peer_product_id.is_none(),
        "Rotation direct adapter received a receiver peer"
    );
    let IdentityLookup::Unique(identity) = device_identity::lookup(target.physical_product_id)
    else {
        anyhow::bail!("Rotation current physical identity is ambiguous");
    };
    ensure!(
        identity.product_id == target.product_id
            && !identity.is_ble
            && source.is_physical_branch(target.physical_product_id, identity.is_dongle),
        "Rotation physical branch differs from its current factory"
    );
    if let Some(change) = change {
        change.validate()?;
    }
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Rotation device task deadline expired"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Rotation device instance changed during communication"
        );
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    let bytes = before["feature_report_bytes"]
        .as_u64()
        .context("Rotation Feature report length is absent")? as usize;
    ensure!(
        bytes == cap.report_bytes,
        "Rotation Feature report length differs"
    );
    let device = hid_transport::open(&target.path, bytes)?;
    let mut next = || transaction(target, &cap);
    if identity.is_dongle {
        razer_device::mouse_current_dpi::verify_wireless_online_for(
            target.product_id,
            &device,
            next()?,
            &validate,
        )?;
    }
    let report = rotation::execute(&device, &cap, change, &mut next, &validate)?;
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
        json!({"target":target,"source_class":source.transport().source_class,
        "transport_source_class":cap.source_class,"report":report,"profile_scope":"rotation_profile_1",
        "transport_metadata":device.metadata()}),
    )
}

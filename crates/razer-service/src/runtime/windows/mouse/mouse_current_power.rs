use super::*;
use razer_device::mouse_current_power::{self as power, Change};
pub(super) fn request(target: &DeviceReadTarget, change: &Change) -> anyhow::Result<Value> {
    let source = power::capability(target.product_id).context("Current power source is absent")?;
    ensure!(
        source.accepts(target.physical_product_id, target.peer_product_id),
        "Power target physical/peer route differs"
    );
    let cap = source.transport(target.physical_product_id, target.peer_product_id.is_some())?;
    power::validate_change(target.product_id, change)?;
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Power confirmation deadline exceeded"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Power device instance changed"
        );
        Ok(())
    };
    let relay = target.peer_product_id;
    let check_peer = || -> anyhow::Result<()> {
        if let Some(peer) = relay {
            let reply = receiver::query(&target.path, &target.device_container_id, None)?;
            ensure!(
                razer_device::receiver_protocol::receiver_peers(&reply)?
                    .iter()
                    .any(|(pid, status)| u32::from(*pid) == peer && *status == 1),
                "Power receiver has no current matching online peer"
            );
        }
        Ok(())
    };
    check_peer()?;
    let report = {
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
        validate()?;
        let bytes = before["feature_report_bytes"]
            .as_u64()
            .context("Power Feature Report size is absent")? as usize;
        ensure!(
            bytes == cap.report_bytes,
            "Power Feature Report size differs"
        );
        let device =
            hid_transport::open_validated(&target.path, bytes, std::rc::Rc::new(&validate))?;
        let mut next = || transaction(target, &cap);
        if relay.is_none() {
            let IdentityLookup::Unique(identity) =
                device_identity::lookup(target.physical_product_id)
            else {
                anyhow::bail!("Power physical identity is ambiguous");
            };
            ensure!(
                identity.product_id == target.product_id && !identity.is_ble,
                "Power physical identity differs"
            );
            if identity.is_dongle {
                power::verify_wireless_online(&device, &cap, next()?, &validate)?;
            }
        }
        let report = power::execute(
            target.product_id,
            &device,
            &cap,
            change,
            &mut next,
            &validate,
        )?;
        if relay.is_none() {
            let IdentityLookup::Unique(identity) =
                device_identity::lookup(target.physical_product_id)
            else {
                anyhow::bail!("Power physical identity is ambiguous after write");
            };
            ensure!(
                identity.product_id == target.product_id && !identity.is_ble,
                "Power physical identity changed after write"
            );
            if identity.is_dongle {
                power::verify_wireless_online(&device, &cap, next()?, &validate)?;
            }
        }
        report
    };
    validate()?;
    check_peer()?;
    Ok(
        json!({"target":target,"source_class":cap.source_class,"report":report,"setting_scope":"basic_device",
        "elapsed_ms":started.elapsed().as_millis() as u64}),
    )
}

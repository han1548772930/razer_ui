use super::*;
use razer_device::{
    mouse_current_dpi as factory,
    mouse_scroll::{MouseScroll, Submission},
};

pub(super) fn request(target: &DeviceReadTarget, submission: &Submission) -> anyhow::Result<Value> {
    submission.validate(target.product_id)?;
    let source =
        factory::capability(target.product_id).context("Current scroll factory is absent")?;
    let cap = source
        .transport()
        .for_physical_product(target.physical_product_id)?;
    ensure!(
        target.peer_product_id.is_none() && cap.direct_pids.contains(&target.physical_product_id),
        "Scroll target physical route differs"
    );
    let IdentityLookup::Unique(identity) = device_identity::lookup(target.physical_product_id)
    else {
        anyhow::bail!("Scroll physical identity is ambiguous");
    };
    ensure!(
        identity.product_id == target.product_id
            && !identity.is_ble
            && source.is_physical_branch(target.physical_product_id, identity.is_dongle),
        "Scroll physical branch differs"
    );
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Scroll confirmation deadline exceeded"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Scroll device instance changed"
        );
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    validate()?;
    let bytes = before["feature_report_bytes"]
        .as_u64()
        .context("Scroll Feature Report size is absent")? as usize;
    ensure!(
        bytes == cap.report_bytes,
        "Scroll Feature Report size differs"
    );
    let device = hid_transport::open(&target.path, bytes)?;
    let mut next = || transaction(target, &cap);
    if identity.is_dongle {
        factory::verify_wireless_online_for(target.product_id, &device, next()?, &validate)?;
    }
    let report = MouseScroll::new(&device, &cap)?.submit(submission, &mut next, &validate)?;
    if identity.is_dongle {
        factory::verify_wireless_online_for(target.product_id, &device, next()?, &validate)?;
    }
    validate()?;
    Ok(
        json!({"target":target,"report":report,"source_class":cap.source_class,"setting_scope":"source_scroll_profile",
        "transport_metadata":device.metadata(),"elapsed_ms":started.elapsed().as_millis() as u64}),
    )
}

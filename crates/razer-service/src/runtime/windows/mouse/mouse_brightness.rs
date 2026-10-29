use super::*;
use razer_device::{
    mouse_brightness::{self as brightness, Submission},
    mouse_current_dpi as factory,
};

pub(super) fn request(target: &DeviceReadTarget, submission: &Submission) -> anyhow::Result<Value> {
    submission.validate(target.product_id)?;
    let source = factory::capability(target.product_id)
        .context("Current brightness physical factory is absent")?;
    let cap = source
        .transport()
        .for_physical_product(target.physical_product_id)?;
    ensure!(
        target.peer_product_id.is_none(),
        "Direct brightness adapter received a receiver peer"
    );
    let IdentityLookup::Unique(identity) = device_identity::lookup(target.physical_product_id)
    else {
        anyhow::bail!("Brightness physical identity is ambiguous");
    };
    ensure!(
        identity.product_id == target.product_id
            && !identity.is_ble
            && source.is_physical_branch(target.physical_product_id, identity.is_dongle),
        "Brightness physical branch differs from its current factory"
    );
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(60),
            "Brightness device task deadline expired"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Brightness device instance changed during communication"
        );
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    let bytes = before["feature_report_bytes"]
        .as_u64()
        .context("Brightness Feature report length is absent")? as usize;
    ensure!(
        bytes == cap.report_bytes,
        "Brightness Feature report length differs"
    );
    let device = hid_transport::open_validated(&target.path, bytes, std::rc::Rc::new(&validate))?;
    let mut next = || transaction(target, &cap);
    if identity.is_dongle {
        factory::verify_wireless_online_for(target.product_id, &device, next()?, &validate)?;
    }
    let report = brightness::execute(&device, &cap, submission, &mut next, &validate)?;
    if identity.is_dongle {
        factory::verify_wireless_online_for(target.product_id, &device, next()?, &validate)?;
    }
    validate()?;
    Ok(
        json!({"target":target,"report":report,"source_class":source.transport().source_class,"transport_source_class":cap.source_class,"setting_scope":"active_and_matching_obm_brightness","transport_metadata":device.metadata()}),
    )
}

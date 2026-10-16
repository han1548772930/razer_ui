use super::*;
use razer_device::mouse_receiver_legacy::{self as indicator, Change, Target};
pub(super) fn request(target: &DeviceReadTarget, change: &Change) -> anyhow::Result<Value> {
    let cap = indicator::change_capability(target.product_id, target.physical_product_id, change)
        .context("Current mouse receiver source is absent")?;
    ensure!(
        indicator::accepts_change(
            target.product_id,
            target.physical_product_id,
            target.peer_product_id,
            change
        ),
        "Mouse receiver command has no current physical route"
    );
    let before = current_target(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Single LED request expired"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Single LED receiver instance changed"
        );
        Ok(())
    };
    let report = {
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        validate()?;
        ensure!(
            before["feature_report_bytes"] == cap.report_bytes,
            "Single LED Feature size differs"
        );
        let device = hid_transport::open(&target.path, cap.report_bytes)?;
        indicator::execute(&device, cap, change, || transaction(target, cap), &validate)?
    };
    validate()?;
    Ok(json!({"target":Target::WindowsContainer(target.clone()),"report":report}))
}

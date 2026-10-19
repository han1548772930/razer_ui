use super::*;
use razer_device::keyboard_legacy_calibration::{self as calibration, Operation, Target};
pub(super) fn request(target: &DeviceReadTarget, operation: &Operation) -> anyhow::Result<Value> {
    ensure!(
        target.peer_product_id.is_none() && target.product_id == target.physical_product_id,
        "Magnetic calibration cannot target a receiver peer"
    );
    let cap = calibration::capability(target.product_id)?;
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Calibration task expired"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Calibration keyboard instance changed"
        );
        Ok(())
    };
    let report = {
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
        validate()?;
        ensure!(
            before["feature_report_bytes"] == cap.report_bytes,
            "Calibration Feature size differs"
        );
        let device = hid_transport::open(&target.path, cap.report_bytes)?;
        calibration::execute(
            &device,
            &cap,
            operation,
            || transaction(target, &cap),
            &validate,
        )?
    };
    validate()?;
    Ok(json!({"target":Target::WindowsContainer(target.clone()),"report":report}))
}

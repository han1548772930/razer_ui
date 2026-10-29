//! Container identity validation and local ReceiverLock stay on Windows.
use super::*;
use razer_device::keyboard_indicator::{self as indicator, Change, Target};
pub(super) fn request(target: &DeviceReadTarget, change: &Change) -> anyhow::Result<Value> {
    ensure!(
        target.peer_product_id.is_none(),
        "Keyboard indicator targets the receiver itself"
    );
    let cap = indicator::capability(target.product_id, target.physical_product_id)?;
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Keyboard indicator task expired"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Keyboard indicator receiver instance changed"
        );
        Ok(())
    };
    let report = {
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
        validate()?;
        ensure!(
            before["feature_report_bytes"] == cap.report_bytes,
            "Keyboard indicator Feature size differs"
        );
        let device = hid_transport::open_validated(
            &target.path,
            cap.report_bytes,
            std::rc::Rc::new(&validate),
        )?;
        indicator::execute(
            &device,
            &cap,
            change,
            || transaction(target, &cap),
            &validate,
        )?
    };
    validate()?;
    Ok(json!({"target":Target::WindowsContainer(target.clone()),"report":report}))
}

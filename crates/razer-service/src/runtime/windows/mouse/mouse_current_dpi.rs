use super::*;
use razer_device::mouse_current_dpi::{self as dpi, DpiChange};

pub(super) fn request(
    target: &DeviceReadTarget,
    change: Option<&DpiChange>,
) -> anyhow::Result<Value> {
    run(target, Operation::Dpi(change))
}

pub(super) fn wireless_online(target: &DeviceReadTarget) -> anyhow::Result<()> {
    run(target, Operation::Online).map(|_| ())
}

enum Operation<'a> {
    Online,
    Dpi(Option<&'a DpiChange>),
}

fn run(target: &DeviceReadTarget, operation: Operation<'_>) -> anyhow::Result<Value> {
    let source = dpi::capability(target.product_id)
        .context("Mouse DPI target has no current source factory capability")?;
    let cap = source
        .transport()
        .for_physical_product(target.physical_product_id)?;
    ensure!(
        cap.direct_pids.contains(&target.physical_product_id) && target.peer_product_id.is_none(),
        "Current mouse DPI target differs from the source's single-device adapter"
    );
    let is_dongle = source.is_dongle(target.physical_product_id)?;
    if let Operation::Dpi(Some(change)) = operation {
        dpi::validate_change_for(target.product_id, change)?;
    }
    if matches!(operation, Operation::Online) {
        ensure!(
            is_dongle,
            "Wireless qualification requires an observed dongle"
        );
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
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    let feature_bytes = before["feature_report_bytes"]
        .as_u64()
        .context("Current mouse DPI Feature Report size is absent")?
        as usize;
    ensure!(
        feature_bytes == cap.report_bytes,
        "Current mouse DPI observed Feature Report size differs"
    );
    let device =
        hid_transport::open_validated(&target.path, feature_bytes, std::rc::Rc::new(&validate))?;
    let mut next = || transaction(target, &cap);
    if is_dongle {
        dpi::verify_wireless_online_for(target.product_id, &device, next()?, &validate)?;
    }
    let Operation::Dpi(change) = operation else {
        validate()?;
        return Ok(json!({"target":target,"wireless_online":true}));
    };
    let report = dpi::execute_for(target.product_id, &device, change, &mut next, &validate)?;
    if is_dongle {
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

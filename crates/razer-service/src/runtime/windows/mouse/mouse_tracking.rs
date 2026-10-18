//! Container adapter retains the native lock and handle for the complete chain.
use super::*;
use razer_device::mouse_tracking::{self as tracking, SubmissionReport, TrackingSetting};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn request(
    target: &DeviceReadTarget,
    setting: TrackingSetting,
    canceled: &AtomicBool,
) -> anyhow::Result<SubmissionReport> {
    tracking::prepare(
        tracking::capability(target.product_id).context("Orca Plus source is absent")?,
        setting,
    )?;
    let source = razer_device::mouse_current_dpi::capability(target.product_id)
        .context("Tracking physical factory is absent")?;
    let session = target
        .peer_product_id
        .map(|peer| source.receiver_session(target.physical_product_id, peer))
        .transpose()?;
    let direct_cap;
    let cap = if let Some(session) = &session {
        session.transport()
    } else {
        ensure!(
            source.is_physical_branch(target.physical_product_id, false),
            "Tracking requires its source wired or bound radio sender"
        );
        direct_cap = source
            .transport()
            .for_physical_product(target.physical_product_id)?;
        &direct_cap
    };
    let before = current_target(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Tracking operation canceled"
        );
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Tracking operation deadline exceeded"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Tracking device instance changed"
        );
        Ok(())
    };
    validate()?;
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    validate()?;
    ensure!(
        before["feature_report_bytes"] == cap.report_bytes,
        "Tracking Feature Report size differs"
    );
    let device = hid_transport::open(&target.path, cap.report_bytes)?;
    Ok(tracking::submit(
        &device,
        cap,
        setting,
        session.as_ref(),
        || transaction(target, cap),
        &validate,
    ))
}

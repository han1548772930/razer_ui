//! PID 70 container scope retains its native lock through the setter chain.
use super::*;
use razer_device::mouse_philips::{self as philips, Setting, SubmissionReport};
use std::sync::atomic::{AtomicBool, Ordering};

/// The calibration lock and Feature handle stay with the native event session,
/// including cancellation/restoration. They are created and dropped on its
/// service thread; no native handle is transferred between Rust threads.
pub(in crate::runtime) struct MeasurementConnection {
    target: DeviceReadTarget,
    instance: String,
    _lock: receiver::ReceiverLock,
    device: hid_transport::Device,
}
impl MeasurementConnection {
    pub(in crate::runtime) fn open(
        target: &DeviceReadTarget,
    ) -> anyhow::Result<(Self, Option<String>)> {
        let cap = razer_device::mouse_legacy_polling::read_capability();
        philips::validate_transport(cap)?;
        ensure!(
            target.product_id == 70
                && target.physical_product_id == 70
                && target.peer_product_id.is_none(),
            "Philips measurement requires its direct wired target"
        );
        let before = current_target(target, cap)?;
        ensure!(
            before["feature_report_bytes"] == cap.report_bytes,
            "Philips Feature length differs"
        );
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Philips measurement initialization deadline exceeded"
            );
            ensure!(
                current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
                "Philips measurement device instance changed"
            );
            Ok(())
        };
        let lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
        let connection = Self {
            target: target.clone(),
            instance: before["device_instance_id"]
                .as_str()
                .context("Philips HID instance missing")?
                .into(),
            _lock: lock,
            device: hid_transport::open_validated(
                &target.path,
                cap.report_bytes,
                std::rc::Rc::new(&validate),
            )?,
        };
        let initialization_error = razer_device::mouse_legacy_polling::initialize_driver_mode(
            &connection.device,
            cap,
            &mut || transaction(target, cap),
            &|| {
                validate()?;
                connection.validate()
            },
        )
        .err()
        .map(|error| format!("{error:#}"));
        Ok((connection, initialization_error))
    }
    pub(in crate::runtime) fn validate(&self) -> anyhow::Result<()> {
        let cap = razer_device::mouse_legacy_polling::read_capability();
        ensure!(
            current_target(&self.target, cap)?["device_instance_id"] == self.instance,
            "Philips calibration device instance changed"
        );
        Ok(())
    }
    pub(in crate::runtime) fn exchange(
        &self,
        command: philips::measurement::DeviceCommand,
        sent: &mut impl FnMut(),
    ) -> anyhow::Result<Vec<u8>> {
        let cap = razer_device::mouse_legacy_polling::read_capability();
        let began = Instant::now();
        philips::measurement::exchange(
            &self.device,
            cap,
            command,
            transaction(&self.target, cap)?,
            || {
                ensure!(
                    began.elapsed() < Duration::from_secs(20),
                    "Philips command deadline exceeded"
                );
                self.validate()
            },
            sent,
        )
    }
}
pub(super) fn request(
    target: &DeviceReadTarget,
    setting: Setting,
    canceled: &AtomicBool,
) -> anyhow::Result<SubmissionReport> {
    let cap = razer_device::mouse_legacy_polling::read_capability();
    philips::validate_transport(cap)?;
    ensure!(
        target.product_id == philips::PRODUCT_ID
            && target.physical_product_id == philips::PRODUCT_ID
            && target.peer_product_id.is_none(),
        "Philips calibration requires its direct wired target"
    );
    let before = current_target(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Philips calibration canceled"
        );
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Philips calibration deadline exceeded"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Philips calibration device instance changed"
        );
        Ok(())
    };
    validate()?;
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    validate()?;
    ensure!(
        before["feature_report_bytes"] == cap.report_bytes,
        "Philips calibration Feature length differs"
    );
    let device =
        hid_transport::open_validated(&target.path, cap.report_bytes, std::rc::Rc::new(&validate))?;
    Ok(philips::submit(
        &device,
        cap,
        setting,
        || transaction(target, cap),
        &validate,
    ))
}

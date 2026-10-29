//! Windows observes the exact Uma collection; shared Rust owns its protocol.
use super::{hid, hid_transport, receiver};
use anyhow::{Context as _, ensure};
use razer_device::hyperpolling_receiver::{self as uma, Command, Observation, Target};
use serde_json::Value;
use std::time::{Duration, Instant};

pub(super) fn request(
    path: &str,
    container: &str,
    command: Command,
    transaction: u8,
) -> anyhow::Result<Value> {
    let (before, cap) = receiver::select_target(&hid::enumerate_metadata()?, path, container)?;
    ensure!(
        cap.product_id == uma::PRODUCT_ID,
        "Uma request targets another receiver"
    );
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(12),
            "Uma request expired"
        );
        let (current, selected) = receiver::select_target(
            &hid::enumerate_collection(0x1532, uma::PRODUCT_ID, path)?,
            path,
            container,
        )?;
        ensure!(
            current["device_instance_id"] == before["device_instance_id"]
                && selected.product_id == uma::PRODUCT_ID,
            "Uma physical instance changed during the request"
        );
        Ok(())
    };
    let _lock = receiver::ReceiverLock::acquire(container, &validate)?;
    let maximum = before["feature_report_bytes"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .context("Uma Feature length is unavailable")?;
    let device = hid_transport::open_validated(path, maximum, std::rc::Rc::new(&validate))?;
    validate()?;
    let reading = uma::execute(&device, cap, command, transaction, &validate)?;
    let instance = before["device_instance_id"]
        .as_str()
        .context("Uma physical instance is absent")?;
    Ok(serde_json::to_value(
        Observation::new(
            Target::WindowsContainer {
                path: path.into(),
                container: container.into(),
            },
            reading,
        )
        .with_physical_instance(instance.to_owned()),
    )?)
}

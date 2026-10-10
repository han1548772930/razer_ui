//! Windows supplies retained receiver identity; all protocol code is shared.
use super::{hid, hid_transport, receiver};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) fn request(path: &str, container: &str, percent: Option<u8>) -> anyhow::Result<Value> {
    let (before, cap) = receiver::select_target(&hid::enumerate_metadata()?, path, container)?;
    ensure!(
        matches!(cap.product_id, 164 | 241),
        "Receiver brightness product has no current source evidence"
    );
    let _lock = receiver::ReceiverLock::acquire(container)?;
    let feature = before["feature_report_bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .context("Receiver Feature length is unavailable")?;
    let device = hid_transport::open(path, feature)?;
    let started = Instant::now();
    let validate = || -> anyhow::Result<()> {
        ensure!(
            started.elapsed() < Duration::from_secs(10),
            "Receiver brightness exceeded application deadline"
        );
        let (current, current_cap) = receiver::select_target(
            &hid::enumerate_for_product(cap.vendor_id, cap.product_id)?,
            path,
            container,
        )?;
        ensure!(
            current["device_instance_id"] == before["device_instance_id"]
                && current_cap.product_id == cap.product_id,
            "Receiver identity changed during brightness submission"
        );
        Ok(())
    };
    validate()?;
    let next = || receiver::next_transaction(path, container, cap);
    let result = match percent {
        Some(percent) => serde_json::to_value(razer_device::receiver_brightness::apply(
            &device, cap, percent, next, &validate,
        )?)?,
        None => serde_json::to_value(razer_device::receiver_brightness::read(
            &device,
            cap,
            next()?,
            &validate,
        )?)?,
    };
    Ok(
        json!({"query":"receiver_brightness","product_id":cap.product_id,
        "path":path,"device_container_id":container,"device_instance_id":before["device_instance_id"],
        "result":result,"vendor_dll_loaded":false}),
    )
}

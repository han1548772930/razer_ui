//! Static-source-proven direct serial query on the exact receiver collection.
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
pub(super) fn read(path: &str, container: &str) -> anyhow::Result<Value> {
    let (before, cap) =
        super::receiver::select_target(&super::hid::enumerate_metadata()?, path, container)?;
    ensure!(
        matches!(cap.product_id, 164 | 241),
        "No source-audited receiver identity query"
    );
    let _lock = super::receiver::ReceiverLock::acquire(container)?;
    let feature = before["feature_report_bytes"]
        .as_u64()
        .context("Missing actual Feature length")? as usize;
    let device = super::hid_transport::open(path, feature)?;
    let started = Instant::now();
    let validate = || -> anyhow::Result<()> {
        ensure!(
            started.elapsed() < Duration::from_secs(10),
            "Receiver serial query expired"
        );
        let (current, _) = super::receiver::select_target(
            &super::hid::enumerate_for_product(cap.vendor_id, cap.product_id)?,
            path,
            container,
        )?;
        ensure!(
            current["device_instance_id"] == before["device_instance_id"],
            "Receiver instance changed during serial read"
        );
        Ok(())
    };
    validate()?;
    let transaction = super::receiver::next_transaction(path, container, cap)?;
    let serial = razer_device::receiver_identity::read_serial(&device, cap, transaction, validate)?;
    validate()?;
    Ok(
        json!({"query":"receiver_serial","product_id":cap.product_id,"device_container_id":container,
        "device_instance_id":before["device_instance_id"],"path":path,"serial_number":serial,
        "source":"rzDevice25.getSerialNumber / 84816.I_ / 30580.kD [22,0,130]","transaction_id":transaction}),
    )
}

//! Source rzDevice30 Rotation commands share the retained GATT response owner.
use super::mouse_runtime_artwork_adapter::ble::{self as gatt, Apartment, Connection};
use super::receiver;
use anyhow::ensure;
use razer_device::{
    mouse_rotation::{Setting, ble},
    mouse_runtime_artwork::WindowsBleTarget,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

struct Session {
    connection: Connection,
    sequence: u8,
}
impl ble::Transport for Session {
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        payload: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<ble::Acknowledgement> {
        let transaction = self.sequence;
        let frame = ble::frame(transaction, command, profile, payload)?;
        self.sequence = if transaction == 30 { 0 } else { transaction + 1 };
        let data = self.connection.exchange_with_retry(&frame, validate)?;
        validate()?;
        Ok(ble::Acknowledgement {
            command,
            profile,
            transaction,
            payload: payload.to_vec(),
            data,
        })
    }
}

pub(super) fn request(
    target: &WindowsBleTarget,
    change: Option<&Setting>,
) -> anyhow::Result<Value> {
    ensure!(
        ble::accepts(target.product_id, target.physical_product_id),
        "Rotation BLE source factory differs"
    );
    if let Some(change) = change {
        change.validate()?;
    }
    let _apartment = Apartment::new()?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Rotation BLE owner deadline expired"
        );
        gatt::current_target(target)
    };
    validate()?;
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    validate()?;
    let mut session = Session {
        connection: Connection::connect_with_retry(target, &validate)?,
        sequence: 0,
    };
    let result = (|| {
        let edition = ble::Transport::exchange(&mut session, ble::EDITION, 0, &[], &validate)?;
        ensure!(
            edition.data.len() >= 3,
            "Rotation BLE connection edition is truncated"
        );
        let report = ble::execute(
            target.product_id,
            target.physical_product_id,
            &mut session,
            change,
            &validate,
        )?;
        validate()?;
        Ok(
            json!({"target":target,"source_class":"rzDevice30","transport":"windows_gatt",
            "connection_edition":edition,"report":report,"profile_scope":"rotation_profile_1"}),
        )
    })();
    // Finish CCCD/callbacks/device handles before the shared container lock.
    let cleanup = session.connection.finish();
    match (result, cleanup) {
        (Ok(reply), Ok(())) => Ok(reply),
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Rotation BLE cleanup also failed: {cleanup:#}")))
        }
    }
}

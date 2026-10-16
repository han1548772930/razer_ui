//! Current rzDevice30 power branch over the same retained WinRT GATT adapter
//! as mouse artwork. The installed HID parent/address identifies the owner;
//! source requests are serialized through response cleanup for that container.
use super::mouse_runtime_artwork_adapter::ble::{self as gatt, Apartment, Connection};
use super::receiver;
use anyhow::{Context as _, ensure};
use razer_device::{
    mouse_current_power::{self as power, Acknowledgement, Change, ble},
    mouse_runtime_artwork::WindowsBleTarget,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

struct Session {
    connection: Connection,
    sequence: u8,
}
fn pause(duration: Duration, validate: &dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<()> {
    let deadline = Instant::now() + duration;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        validate()?;
        std::thread::sleep(remaining.min(Duration::from_millis(5)));
    }
    validate()
}
impl ble::Transport for Session {
    fn exchange(
        &mut self,
        command: [u8; 3],
        data: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement> {
        let transaction = self.sequence;
        self.sequence = if self.sequence == 30 { 0 } else { self.sequence + 1 };
        let frame = ble::frame(transaction, command, data)?;
        let mut last = None;
        // Current rzDevice30 lI=2/lz=30; retry keeps the same transaction.
        for attempt in 0..2 {
            if attempt > 0 {
                pause(Duration::from_millis(30), validate)?;
            }
            match self.connection.exchange(&frame, validate) {
                Ok(data) => return ble::acknowledged(command, transaction, data),
                Err(error) => {
                    validate()?;
                    if !gatt::retryable(&error) {
                        return Err(error);
                    }
                    last = Some(error);
                }
            }
        }
        Err(last.context("BLE power request did not run")?)
    }
}

pub(super) fn request(target: &WindowsBleTarget, change: &Change) -> anyhow::Result<Value> {
    let source =
        power::capability(target.product_id).context("Current BLE power source is absent")?;
    ensure!(
        source.is_ble(target.physical_product_id),
        "BLE power physical PID differs"
    );
    power::validate_change(target.product_id, change)?;
    let _apartment = Apartment::new()?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "BLE power confirmation deadline exceeded"
        );
        gatt::current_target(target)
    };
    validate()?;
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
    validate()?;
    let mut connection = None;
    let mut last = None;
    // Current rzDevice30 connectDevice w6=2/ah=150.
    for attempt in 0..2 {
        if attempt > 0 {
            pause(Duration::from_millis(150), &validate)?;
        }
        match Connection::connect(target, &validate) {
            Ok(opened) => {
                connection = Some(opened);
                break;
            }
            Err(error) => {
                validate()?;
                last = Some(error);
            }
        }
    }
    let mut session = Session {
        connection: match connection {
            Some(connection) => connection,
            None => return Err(last.context("BLE power connection did not run")?),
        },
        sequence: 0,
    };
    let result = (|| {
        // Source connectDevice requires a real Get Edition Information ACK
        // before declaring the Protocol30 command channel connected.
        let edition = ble::Transport::exchange(&mut session, ble::EDITION, &[], &validate)?;
        ensure!(
            edition.data.len() >= 3,
            "BLE connection edition response is truncated"
        );
        let report = ble::execute(target.product_id, &mut session, change, &validate)?;
        validate()?;
        Ok(
            json!({"target":target,"source_class":"rzDevice30","transport":"windows_gatt",
            "connection_edition":edition,"report":report,"setting_scope":"basic_device",
            "elapsed_ms":started.elapsed().as_millis() as u64}),
        )
    })();
    let cleanup = session.connection.finish();
    match (result, cleanup) {
        (Ok(reply), Ok(())) => Ok(reply),
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("BLE power cleanup also failed: {cleanup:#}")))
        }
    }
}

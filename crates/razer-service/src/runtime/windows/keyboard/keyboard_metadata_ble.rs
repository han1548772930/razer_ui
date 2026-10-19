use super::mouse_runtime_artwork_adapter::ble::{self as gatt, Apartment, Connection};
use super::receiver;
use anyhow::{Context as _, ensure};
use razer_device::{
    keyboard_metadata::{self, BleTransport},
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

impl BleTransport for Session {
    fn exchange(
        &mut self,
        command: [u8; 3],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>> {
        let frame = keyboard_metadata::ble_frame(self.sequence, command)?;
        self.sequence = (self.sequence + 1) % 31;
        let mut last_error = None;
        for attempt in 0..2 {
            if attempt > 0 {
                pause(Duration::from_millis(30), validate)?;
            }
            match self.connection.exchange(&frame, validate) {
                Ok(data) => return Ok(data),
                Err(error) => {
                    validate()?;
                    if !gatt::retryable(&error) {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.context("Keyboard BLE metadata command did not run")?)
    }
}

pub(super) fn request(target: &WindowsBleTarget, serial: bool) -> anyhow::Result<Value> {
    ensure!(
        keyboard_metadata::ble_product_id(target.product_id) == Some(target.physical_product_id),
        "Keyboard BLE metadata physical identity differs from source"
    );
    let _apartment = Apartment::new()?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Keyboard BLE metadata deadline expired"
        );
        gatt::current_target(target)
    };
    validate()?;
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    let mut connection = None;
    let mut last_error = None;
    for _ in 0..2 {
        match Connection::connect(target, &validate) {
            Ok(opened) => {
                connection = Some(opened);
                break;
            }
            Err(error) => {
                validate()?;
                last_error = Some(error);
                pause(Duration::from_millis(150), &validate)?;
            }
        }
    }
    let mut session = Session {
        connection: match connection {
            Some(opened) => opened,
            None => return Err(last_error.context("Keyboard BLE connection did not run")?),
        },
        sequence: 0,
    };
    let result = (|| {
        let connection_edition = session.exchange([0, 1, 134], &validate)?;
        ensure!(
            connection_edition.len() >= 2,
            "Keyboard BLE connection edition response is truncated"
        );
        let mut response = json!({"target": target, "transport": "windows_gatt", "connection_edition": connection_edition});
        if serial {
            response["method"] = json!("getSerialNumber");
            response["serial_number"] = json!(keyboard_metadata::read_ble_serial(
                target.product_id,
                &mut session,
                &validate
            )?);
        } else {
            response["method"] = json!("getEdition");
            response["reading"] = serde_json::to_value(keyboard_metadata::read_ble(
                target.product_id,
                &mut session,
                &validate,
            )?)?;
        }
        validate()?;
        response["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        Ok(response)
    })();
    let cleanup = session.connection.finish();
    match (result, cleanup) {
        (Ok(reply), Ok(())) => Ok(reply),
        (Ok(_), Err(error)) | (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "Keyboard BLE metadata cleanup also failed: {cleanup:#}"
        ))),
    }
}

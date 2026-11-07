use super::mouse_runtime_artwork_adapter::ble::{self as gatt, Apartment, Connection};
use super::receiver;
use anyhow::ensure;
use razer_device::{
    keyboard_settings::{
        BrightnessSubmission,
        ble::{self, Acknowledgement, Transport},
    },
    mouse_runtime_artwork::WindowsBleTarget,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

struct Session {
    connection: Connection,
    sequence: u8,
}
impl Transport for Session {
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        selector: u8,
        data: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement> {
        let transaction = self.sequence;
        self.sequence = (transaction + 1) % 31;
        let frame = ble::frame(transaction, command, profile, selector, data)?;
        let data = self.connection.exchange_with_retry(&frame, validate)?;
        Acknowledgement::new(transaction, command, profile, selector, data)
    }
}

pub(super) fn request(
    target: &WindowsBleTarget,
    submission: Option<BrightnessSubmission>,
) -> anyhow::Result<Value> {
    ensure!(
        ble::available(target.product_id, target.physical_product_id),
        "Keyboard BLE brightness physical identity differs from source"
    );
    if let Some(submission) = submission {
        submission.validate()?;
    }
    let _apartment = Apartment::new()?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Keyboard BLE brightness confirmation deadline expired"
        );
        gatt::current_target(target)
    };
    validate()?;
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    let mut session = Session {
        connection: Connection::connect_with_retry(target, &validate)?,
        sequence: 0,
    };
    let mut progress = None;
    let result = (|| {
        let edition = session.exchange(ble::EDITION, 0, 0, &[], &validate)?;
        ensure!(
            edition.data().len() >= 2,
            "Keyboard BLE connection edition response is truncated"
        );
        let report = ble::execute(
            target.product_id,
            target.physical_product_id,
            &mut session,
            submission,
            &validate,
        )?;
        progress = Some(report.clone());
        validate()?;
        Ok(
            json!({"target":target,"transport":"windows_gatt","source_class":"rzDevice30",
            "connection_edition":edition,"report":report}),
        )
    })();
    // The response is not delivered until notify registrations and GATT
    // operations have completed their owned release path.
    let cleanup = session.connection.finish();
    match (result, cleanup) {
        (Ok(reply), Ok(())) => Ok(reply),
        (Ok(_), Err(error)) | (Err(error), Ok(())) => Err(retain_progress(error, progress)),
        (Err(error), Err(cleanup)) => Err(retain_progress(
            error.context(format!(
                "Keyboard BLE brightness cleanup also failed: {cleanup:#}"
            )),
            progress,
        )),
    }
}

fn retain_progress(error: anyhow::Error, progress: Option<ble::Report>) -> anyhow::Error {
    match progress {
        Some(report) => error.context(ble::Failure::new(report)),
        None => error,
    }
}

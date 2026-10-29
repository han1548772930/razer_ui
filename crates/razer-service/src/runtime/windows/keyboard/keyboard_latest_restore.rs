use super::mouse_runtime_artwork_adapter::ble::{self as gatt, Apartment, Connection};
use super::*;
use razer_device::keyboard_latest_protocol::{self as protocol, Operation, Target};

pub(super) fn event_source(
    target: &Target,
) -> anyhow::Result<Box<dyn razer_device::receiver_pairing::HardwareEvents>> {
    let (physical, container) = match target {
        Target::WindowsContainer(target) => {
            ensure!(
                target.product_id == 756 && target.peer_product_id.is_none(),
                "756 input event adapter logical identity differs"
            );
            let cap = protocol::capability(target.physical_product_id)?;
            current_target(target, cap)?;
            (
                target.physical_product_id,
                target.device_container_id.as_str(),
            )
        }
        Target::WindowsBle(target) => {
            ensure!(
                target.product_id == 756 && target.physical_product_id == 758,
                "756 BLE event identity differs"
            );
            gatt::current_target(target)?;
            (758, target.device_container_id.as_str())
        }
        Target::Collection(_) => {
            anyhow::bail!("Collection events use their portable interrupt owner")
        }
    };
    Ok(Box::new(super::super::receiver_events::Events::new(
        physical as u16,
        container,
    )))
}

fn pause(ms: u64, validate: &dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<()> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(ms) {
        validate()?;
        std::thread::sleep(Duration::from_millis(5));
    }
    validate()
}
pub(super) fn request(target: &Target, operation: &Operation) -> anyhow::Result<Value> {
    match target {
        Target::WindowsContainer(hid_target) => {
            ensure!(
                hid_target.product_id == 756 && hid_target.peer_product_id.is_none(),
                "756 latest restore cannot target a cached foreign receiver"
            );
            let cap = protocol::capability(hid_target.physical_product_id)?;
            let before = current_target(hid_target, cap)?;
            let started = Instant::now();
            let validate = || {
                ensure!(
                    started.elapsed() < Duration::from_secs(120),
                    "756 restore operation expired"
                );
                ensure!(
                    current_target(hid_target, cap)?["device_instance_id"]
                        == before["device_instance_id"],
                    "756 restore device instance changed"
                );
                Ok(())
            };
            let report = {
                let _lock =
                    receiver::ReceiverLock::acquire(&hid_target.device_container_id, &validate)?;
                validate()?;
                ensure!(
                    before["feature_report_bytes"] == cap.report_bytes,
                    "756 restore Feature length differs"
                );
                let device = hid_transport::open_validated(
                    &hid_target.path,
                    cap.report_bytes,
                    std::rc::Rc::new(&validate),
                )?;
                protocol::execute_hid(
                    &device,
                    cap,
                    operation,
                    || transaction(hid_target, cap),
                    &validate,
                )?
            };
            validate()?;
            Ok(json!({"target":target,"report":report}))
        }
        Target::WindowsBle(ble_target) => {
            ensure!(
                ble_target.product_id == 756 && ble_target.physical_product_id == 758,
                "756 BLE profile adapter identity differs"
            );
            ensure!(
                matches!(
                    operation,
                    Operation::BleSlot { .. }
                        | Operation::ReadTimeToDim { .. }
                        | Operation::ReadAutoSleepMode { .. }
                        | Operation::ReadTimeToSleep
                ),
                "756 BLE operation has no current protocol30 adapter"
            );
            let _apartment = Apartment::new()?;
            let started = Instant::now();
            let validate = || {
                ensure!(
                    started.elapsed() < Duration::from_secs(20),
                    "756 BLE profile operation expired"
                );
                gatt::current_target(ble_target)
            };
            validate()?;
            let _lock =
                receiver::ReceiverLock::acquire(&ble_target.device_container_id, &validate)?;
            let mut opened = None;
            let mut last_error = None;
            for attempt in 0..2 {
                if attempt > 0 {
                    pause(150, &validate)?;
                }
                match Connection::connect(ble_target, &validate) {
                    Ok(connection) => {
                        opened = Some(connection);
                        break;
                    }
                    Err(error) => {
                        if !gatt::connection_retryable(&error) {
                            return Err(error);
                        }
                        validate()?;
                        last_error = Some(error);
                    }
                }
            }
            let mut connection = opened.ok_or_else(|| {
                last_error.unwrap_or_else(|| anyhow::anyhow!("756 BLE connect did not run"))
            })?;
            let result = (|| -> anyhow::Result<Value> {
                // Current rzBLE connection probes edition before its protocol30 adapter is usable.
                let edition = connection.exchange(&[0, 0, 0, 0, 1, 134, 0, 0], &validate)?;
                ensure!(edition.len() >= 2, "756 BLE edition response is truncated");
                let mut sequence = 1;
                let report =
                    protocol::execute_ble(operation, |command, profile, element, data| {
                        validate()?;
                        let frame = protocol::ble_frame(sequence, command, profile, element, data)?;
                        sequence = if sequence == 30 { 0 } else { sequence + 1 };
                        let mut last_error = None;
                        // Source retries preserve the command's transaction ID.
                        for attempt in 0..2 {
                            if attempt > 0 {
                                pause(30, &validate)?;
                            }
                            match connection.exchange(&frame, &validate) {
                                Ok(reply) => {
                                    validate()?;
                                    return Ok(reply);
                                }
                                Err(error) => {
                                    validate()?;
                                    if !gatt::retryable(&error) {
                                        return Err(error);
                                    }
                                    last_error = Some(error);
                                }
                            }
                        }
                        Err(last_error
                            .unwrap_or_else(|| anyhow::anyhow!("756 BLE command did not run")))
                    })?;
                validate()?;
                Ok(json!({"target":target,"report":report}))
            })();
            let cleanup = connection.finish();
            match (result, cleanup) {
                (Ok(reply), Ok(())) => Ok(reply),
                (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
                (Err(error), Err(cleanup)) => {
                    Err(error.context(format!("756 BLE cleanup also failed: {cleanup:#}")))
                }
            }
        }
        Target::Collection(_) => {
            anyhow::bail!("HID collections use the portable keyboard restore adapter")
        }
    }
}

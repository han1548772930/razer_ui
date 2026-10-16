//! Current mouse rzDevice30 battery members selected by physical bleId.
//! Protocol30 uses class 5 and little-endian sleep seconds. Setter readbacks
//! confirm the source task; they never replace optimistic UI or local drafts.
use super::{Acknowledgement, Change, Report, capability, validate_change};
use crate::mouse_runtime_artwork::WindowsBleTarget;
use anyhow::{Context as _, ensure};

pub const EDITION: [u8; 3] = [0, 1, 134];
pub const READ_SLEEP: [u8; 3] = [0, 5, 132];
pub const SET_SLEEP: [u8; 3] = [2, 5, 4];
pub const READ_CRITICAL: [u8; 3] = [0, 5, 130];
pub const SET_CRITICAL: [u8; 3] = [1, 5, 2];
pub const SET_BATTERY_TYPE: [u8; 3] = [1, 5, 15];

pub fn target_from_hid(
    product_id: u32,
    physical_product_id: u32,
    container: &str,
    observation: &serde_json::Value,
) -> anyhow::Result<WindowsBleTarget> {
    let source = capability(product_id).context("Current mouse power source is absent")?;
    ensure!(
        source.is_ble(physical_product_id),
        "Current power source has no such BLE identity"
    );
    WindowsBleTarget::from_hid(product_id, physical_product_id, container, observation)
}

pub fn frame(transaction: u8, command: [u8; 3], data: &[u8]) -> anyhow::Result<Vec<u8>> {
    ensure!(transaction < 31, "Protocol30 transaction is invalid");
    ensure!(
        usize::from(command[0]) == data.len(),
        "Protocol30 power payload length differs"
    );
    let mut frame = vec![transaction, command[0], 0, 0, command[1], command[2], 0, 0];
    frame.extend_from_slice(data);
    Ok(frame)
}

/// Called by a platform transport only after its actual response is assembled.
pub fn acknowledged(
    command: [u8; 3],
    transaction: u8,
    data: Vec<u8>,
) -> anyhow::Result<Acknowledgement> {
    ensure!(
        transaction < 31,
        "Protocol30 acknowledgement transaction is invalid"
    );
    Ok(Acknowledgement {
        command,
        transaction,
        data,
    })
}

pub trait Transport {
    fn exchange(
        &mut self,
        command: [u8; 3],
        data: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement>;
}

fn exchange(
    transport: &mut dyn Transport,
    command: [u8; 3],
    data: &[u8],
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Acknowledgement> {
    validate()?;
    let acknowledgement = transport.exchange(command, data, validate)?;
    validate()?;
    ensure!(
        acknowledgement.command == command && acknowledgement.transaction < 31,
        "BLE power acknowledgement belongs to another request"
    );
    Ok(acknowledgement)
}

pub fn execute(
    product_id: u32,
    transport: &mut dyn Transport,
    change: &Change,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    validate_change(product_id, change)?;
    let source = capability(product_id).expect("validated current power source");
    ensure!(
        source.ble_id.is_some_and(|pid| pid != 0),
        "Current power product has no mounted BLE factory branch"
    );
    let mut report = Report {
        requested: change.clone(),
        acknowledgements: vec![],
        time_to_sleep: None,
        critical_percent: None,
        dim_before: None,
        setter_sent: false,
    };
    match *change {
        Change::ReadPowerSaving => {
            let reply = exchange(transport, READ_SLEEP, &[], validate)?;
            ensure!(
                reply.data.len() >= 2,
                "BLE time-to-sleep initialization response is truncated"
            );
            report.time_to_sleep = Some(u16::from_le_bytes([reply.data[0], reply.data[1]]));
            report.acknowledgements.push(reply);
        }
        Change::PowerSaving(value) => {
            let seconds = if source.single_profile_seconds {
                value
            } else {
                value
                    .checked_mul(60)
                    .context("BLE power saving seconds overflow")?
            };
            report.acknowledgements.push(exchange(
                transport,
                SET_SLEEP,
                &seconds.to_le_bytes(),
                validate,
            )?);
            report.setter_sent = true;
            let reply = exchange(transport, READ_SLEEP, &[], validate)?;
            ensure!(
                reply.data.len() >= 2,
                "BLE time-to-sleep readback is truncated"
            );
            let actual = u16::from_le_bytes([reply.data[0], reply.data[1]]);
            report.time_to_sleep = Some(actual);
            report.acknowledgements.push(reply);
            ensure!(
                actual == seconds,
                "BLE power saving source task readback does not match"
            );
        }
        Change::LowPower(percent) | Change::LowBattery(percent) => {
            let raw = (u16::from(percent) * 255).div_ceil(100) as u8;
            report
                .acknowledgements
                .push(exchange(transport, SET_CRITICAL, &[raw], validate)?);
            report.setter_sent = true;
            let reply = exchange(transport, READ_CRITICAL, &[], validate)?;
            let raw = *reply
                .data
                .first()
                .context("BLE critical-level readback is truncated")?;
            let actual = (u16::from(raw) * 100 / 255) as u8;
            report.critical_percent = Some(actual);
            report.acknowledgements.push(reply);
            ensure!(
                actual == percent,
                "BLE critical-level source task readback does not match"
            );
        }
        Change::BatteryType(value) => {
            // Current taskRunner batteryType completes from this setter ACK.
            report.acknowledgements.push(exchange(
                transport,
                SET_BATTERY_TYPE,
                &[value],
                validate,
            )?);
            report.setter_sent = true;
        }
        Change::DimLighting { .. } => {
            anyhow::bail!("No current BLE power product mounts the dim-lighting control")
        }
    }
    validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, collections::VecDeque};

    struct Script {
        replies: VecDeque<Vec<u8>>,
        frames: Vec<Vec<u8>>,
    }
    impl Transport for Script {
        fn exchange(
            &mut self,
            command: [u8; 3],
            data: &[u8],
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<Acknowledgement> {
            validate()?;
            let transaction = self.frames.len() as u8;
            self.frames.push(frame(transaction, command, data)?);
            Ok(Acknowledgement {
                command,
                transaction,
                data: self.replies.pop_front().expect("scripted actual response"),
            })
        }
    }
    fn script(replies: impl IntoIterator<Item = Vec<u8>>) -> Script {
        Script {
            replies: replies.into_iter().collect(),
            frames: vec![],
        }
    }

    #[test]
    fn ble_power_sleep_keeps_seconds_and_little_endian_payload() {
        let mut io = script([vec![], vec![60, 0]]);
        let report = execute(199, &mut io, &Change::PowerSaving(60), &|| Ok(())).unwrap();
        assert_eq!(
            io.frames,
            [
                vec![0, 2, 0, 0, 5, 4, 0, 0, 60, 0],
                vec![1, 0, 0, 0, 5, 132, 0, 0]
            ]
        );
        assert_eq!(report.time_to_sleep, Some(60));
        assert!(report.setter_sent);
        let mut io = script([vec![44, 1]]);
        let report = execute(143, &mut io, &Change::ReadPowerSaving, &|| Ok(())).unwrap();
        assert_eq!(report.time_to_sleep, Some(300));
        assert!(!report.setter_sent);
        assert_eq!(io.frames, [vec![0, 0, 0, 0, 5, 132, 0, 0]]);
        assert_eq!(frame(30, READ_SLEEP, &[]).unwrap()[0], 30);
        assert!(frame(31, READ_SLEEP, &[]).is_err());
        let mut io = script([vec![], vec![44, 1]]);
        execute(143, &mut io, &Change::PowerSaving(5), &|| Ok(())).unwrap();
        assert_eq!(io.frames[0], [0, 2, 0, 0, 5, 4, 0, 0, 44, 1]);
    }

    #[test]
    fn ble_power_critical_and_battery_type_follow_distinct_source_tasks() {
        let mut io = script([vec![], vec![128]]);
        let report = execute(143, &mut io, &Change::LowPower(50), &|| Ok(())).unwrap();
        assert_eq!(report.critical_percent, Some(50));
        assert_eq!(
            io.frames,
            [
                vec![0, 1, 0, 0, 5, 2, 0, 0, 128],
                vec![1, 0, 0, 0, 5, 130, 0, 0]
            ]
        );
        let mut io = script([vec![]]);
        let report = execute(224, &mut io, &Change::BatteryType(2), &|| Ok(())).unwrap();
        assert_eq!(io.frames, [vec![0, 1, 0, 0, 5, 15, 0, 0, 2]]);
        assert!(report.setter_sent);
    }

    #[test]
    fn ble_power_rejects_failed_readback_and_canceled_owner() {
        let mut io = script([vec![], vec![59, 0]]);
        assert!(execute(199, &mut io, &Change::PowerSaving(60), &|| Ok(())).is_err());
        let canceled = Cell::new(false);
        let mut io = script([vec![], vec![60, 0]]);
        let guard = || {
            ensure!(!canceled.replace(true), "owner canceled");
            Ok(())
        };
        assert!(execute(199, &mut io, &Change::PowerSaving(60), &guard).is_err());
        assert!(io.frames.is_empty());
        assert!(execute(158, &mut io, &Change::PowerSaving(5), &|| Ok(())).is_err());
        assert!(!capability(158).unwrap().is_ble(0));
    }

    #[test]
    fn ble_power_owns_battery_product_identity_independently_of_artwork_queries() {
        let container = "{01234567-89AB-CDEF-0123-456789ABCDEF}";
        let observation = serde_json::json!({"complete":true,"interfaces":[{
            "path":r"\\?\hid#mouse#battery",
            "device_container_id":container,
            "device_instance_id":r"HID\BLE_MOUSE\BATTERY",
            "parent_instance_id":r"BTHLEDevice\_VID&021532_PID&00E1_REV&0100_A1B2C3D4E5F6",
            "vendor_id":0x1532,"product_id":225
        }]});
        let target = target_from_hid(224, 225, container, &observation).unwrap();
        assert_eq!(target.mac_address, 0xa1b2c3d4e5f6);
        assert_eq!(target.physical_product_id, 225);
        assert!(target_from_hid(224, 224, container, &observation).is_err());
        assert!(target_from_hid(158, 0, container, &observation).is_err());
    }
}

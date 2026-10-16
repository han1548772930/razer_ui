//! Current mouse basic power tasks over the source Protocol25 Feature layout.
//! The BLE module owns the current factory's separate Protocol30 commands.
//! Local persistence and optimistic UI are owned by the caller. Setter
//! readbacks confirm tasks; only the source's explicit initialization getter
//! supplies a replacement UI value.
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, Deserialize)]
pub struct Capability {
    product_id: u32,
    power_saving: bool,
    low_power: bool,
    low_battery: bool,
    dim_lighting: bool,
    battery_type: bool,
    dongle_id: Option<u32>,
    ble_id: Option<u32>,
    dual: bool,
    mika: bool,
    single_profile_seconds: bool,
    always_read_power_saving: bool,
    transport: DeviceReadCapability,
}
impl Capability {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn is_ble(&self, physical: u32) -> bool {
        self.ble_id.is_some_and(|ble| ble != 0 && ble == physical)
    }
    pub fn accepts(&self, physical: u32, peer: Option<u32>) -> bool {
        if let Some(peer) = peer {
            (self.dual || self.mika && self.dongle_id == Some(physical))
                && (peer == self.product_id || self.dongle_id == Some(peer))
        } else {
            physical == self.product_id || self.dongle_id == Some(physical) || self.is_ble(physical)
        }
    }
    pub fn always_read_power_saving(&self) -> bool {
        self.always_read_power_saving
    }
    pub fn transport(&self, physical: u32, relay: bool) -> anyhow::Result<DeviceReadCapability> {
        ensure!(
            !self.is_ble(physical),
            "Mouse BLE power uses its separate Protocol30 adapter"
        );
        ensure!(
            relay || self.accepts(physical, None),
            "Current power physical branch differs"
        );
        ensure!(
            !relay || self.dual || self.mika && self.dongle_id == Some(physical),
            "Current power source has no receiver relay branch"
        );
        if relay && self.dual {
            let receiver = crate::receiver_capabilities::capability(u16::try_from(physical)?)
                .context("Current power receiver capability is absent")?;
            return self
                .transport
                .for_duallink_master(physical, receiver.claim_interface);
        }
        self.transport.for_physical_product(physical)
    }
}

#[path = "mouse_current_power_ble.rs"]
pub mod ble;
pub fn capability(product_id: u32) -> Option<&'static Capability> {
    static CAPS: OnceLock<Vec<Capability>> = OnceLock::new();
    CAPS.get_or_init(|| {
        serde_json::from_str(include_str!("mouse_current_power_data.json"))
            .expect("current mouse basic-power source capabilities")
    })
    .iter()
    .find(|cap| cap.product_id == product_id)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum Change {
    ReadPowerSaving,
    PowerSaving(u16),
    LowPower(u8),
    LowBattery(u8),
    DimLighting { enabled: bool, minutes: u8 },
    BatteryType(u8),
}
impl Change {
    pub fn key(&self) -> &'static str {
        match self {
            Self::ReadPowerSaving => "powerSavingValue",
            Self::PowerSaving(_) => "powerSavingValue",
            Self::LowPower(_) => "lowPowerMode",
            Self::LowBattery(_) => "lowBatteryEffects",
            Self::DimLighting { .. } => "dimLighting",
            Self::BatteryType(_) => "batteryType",
        }
    }
}
pub fn validate_change(product_id: u32, change: &Change) -> anyhow::Result<()> {
    let cap = capability(product_id).context("Current mouse power source capability is absent")?;
    ensure!(
        match change {
            Change::ReadPowerSaving => cap.power_saving && cap.always_read_power_saving,
            Change::PowerSaving(value) =>
                cap.power_saving
                    && *value > 0
                    && if cap.single_profile_seconds {
                        [15, 30, 60, 120, 300, 600].contains(value)
                    } else {
                        *value <= 15
                    },
            Change::LowPower(value) =>
                cap.low_power && (5..=100).contains(value) && value.is_multiple_of(5),
            Change::LowBattery(value) =>
                cap.low_battery && (5..=100).contains(value) && value.is_multiple_of(5),
            Change::DimLighting { minutes, .. } => cap.dim_lighting && (1..=15).contains(minutes),
            Change::BatteryType(value) => cap.battery_type && *value <= 2,
        },
        "Current mouse power field/range differs from its mounted caller"
    );
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Acknowledgement {
    pub command: [u8; 3],
    pub transaction: u8,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Report {
    pub requested: Change,
    pub acknowledgements: Vec<Acknowledgement>,
    pub time_to_sleep: Option<u16>,
    pub critical_percent: Option<u8>,
    pub dim_before: Option<Vec<u8>>,
    pub setter_sent: bool,
}
fn exchange(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    data: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Acknowledgement> {
    ensure!(
        cap.report_bytes == 91
            && command[0] <= 80
            && data.len() <= usize::from(command[0])
            && transaction & 31 < 31
            && transaction & !31 == cap.transaction_prefix,
        "Current power packet/transaction layout differs"
    );
    let mut outgoing = [0; 91];
    outgoing[0] = cap.report_id;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + data.len()].copy_from_slice(data);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last = "No power response".to_owned();
    for _ in 0..cap.max_retry_out {
        validate()?;
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
        validate()?;
        if let Err(error) = device.send_feature_at_collection_max(&outgoing) {
            last = format!("{error:#}");
            continue;
        }
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = [0; 91];
            incoming[0] = cap.get_report_id_for_transport(device)?;
            let received = device.get_feature_at_collection_max(&mut incoming);
            validate()?;
            let count = match received {
                Ok(count) => count,
                Err(error) => {
                    last = format!("{error:#}");
                    break;
                }
            };
            ensure!(count == 91, "Power response length differs");
            if incoming[2] != transaction || incoming[7..9] != command[1..] {
                last = "Power response transaction/command differs".into();
                break;
            }
            match incoming[1] {
                1 => last = "Power device remains busy".into(),
                2 => {
                    let size = usize::from(incoming[6]);
                    ensure!(size <= 80, "Power response is truncated");
                    return Ok(Acknowledgement {
                        command,
                        transaction,
                        data: incoming[9..9 + size].to_vec(),
                    });
                }
                0 | 3 | 4 => {
                    last = format!("Power response status {}", incoming[1]);
                    break;
                }
                status => anyhow::bail!("Power command rejected with status {status}"),
            }
        }
    }
    anyhow::bail!("Current power source retries exhausted: {last}")
}
pub fn verify_wireless_online(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let reply = exchange(device, cap, [1, 0, 184], &[0], transaction, &validate)?;
    ensure!(
        reply.data.first() == Some(&1),
        "Mouse power receiver has no confirmed online peer"
    );
    Ok(())
}
pub fn execute(
    product_id: u32,
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    change: &Change,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    validate_change(product_id, change)?;
    let source = capability(product_id).expect("validated source");
    let mut report = Report {
        requested: change.clone(),
        acknowledgements: Vec::new(),
        time_to_sleep: None,
        critical_percent: None,
        dim_before: None,
        setter_sent: false,
    };
    match *change {
        Change::ReadPowerSaving => {
            let reply = exchange(device, cap, [2, 7, 131], &[0, 0], next()?, &validate)?;
            ensure!(
                reply.data.len() >= 2,
                "Time-to-sleep initialization response is truncated"
            );
            report.time_to_sleep = Some(u16::from_be_bytes([reply.data[0], reply.data[1]]));
            report.acknowledgements.push(reply);
        }
        Change::PowerSaving(value) => {
            let seconds = if source.single_profile_seconds {
                value
            } else {
                value
                    .checked_mul(60)
                    .context("Power saving seconds overflow")?
            };
            report.acknowledgements.push(exchange(
                device,
                cap,
                [2, 7, 3],
                &seconds.to_be_bytes(),
                next()?,
                &validate,
            )?);
            report.setter_sent = true;
            let reply = exchange(device, cap, [2, 7, 131], &[0, 0], next()?, &validate)?;
            ensure!(reply.data.len() >= 2, "Time-to-sleep readback is truncated");
            let actual = u16::from_be_bytes([reply.data[0], reply.data[1]]);
            report.time_to_sleep = Some(actual);
            report.acknowledgements.push(reply);
            ensure!(
                actual == seconds,
                "Power saving source task readback does not match"
            );
        }
        Change::LowPower(percent) | Change::LowBattery(percent) => {
            // Source lowBatteryEffects and lowPowerMode both select aLH's
            // setCriticalLevel/getCriticalLevel runner; do not invent LED writes.
            let raw = (u16::from(percent) * 255).div_ceil(100) as u8;
            report.acknowledgements.push(exchange(
                device,
                cap,
                [1, 7, 1],
                &[raw],
                next()?,
                &validate,
            )?);
            report.setter_sent = true;
            let reply = exchange(device, cap, [1, 7, 129], &[0], next()?, &validate)?;
            let raw = *reply
                .data
                .first()
                .context("Critical-level readback is truncated")?;
            let actual = (u16::from(raw) * 100 / 255) as u8;
            report.critical_percent = Some(actual);
            report.acknowledgements.push(reply);
            ensure!(
                actual == percent,
                "Critical-level source task readback does not match"
            );
        }
        Change::DimLighting { enabled, minutes } => {
            let before = exchange(
                device,
                cap,
                [5, 7, 139],
                &[0, 0, 0, 0, 0],
                next()?,
                &validate,
            )?;
            ensure!(
                before.data.len() >= 5 && before.data[0] == 0,
                "Dim getter response is truncated or belongs to another profile"
            );
            let seconds = u16::from(minutes) * 60;
            let differs = u16::from_be_bytes([before.data[2], before.data[3]]) != seconds
                || (before.data[1] == 1) != enabled;
            report.dim_before = Some(before.data.clone());
            report.acknowledgements.push(before);
            if differs {
                let [hi, lo] = seconds.to_be_bytes();
                report.acknowledgements.push(exchange(
                    device,
                    cap,
                    [5, 7, 11],
                    &[0, u8::from(enabled), hi, lo, 20],
                    next()?,
                    &validate,
                )?);
                report.setter_sent = true;
            }
        }
        Change::BatteryType(value) => {
            report.acknowledgements.push(exchange(
                device,
                cap,
                [1, 7, 20],
                &[value],
                next()?,
                &validate,
            )?);
            report.setter_sent = true;
        }
    }
    validate()?;
    Ok(report)
}

#[cfg(test)]
#[path = "mouse_current_power_tests.rs"]
mod tests;

//! Current mouse basic power tasks over the source Protocol25 Feature layout.
//! The BLE module owns the current factory's separate Protocol30 commands.
//! Local persistence and optimistic UI are owned by the caller. Setter
//! readbacks confirm tasks; only the source's explicit initialization getter
//! supplies a replacement UI value.
use crate::{
    backend::FeatureTransport,
    device_reads::{DeviceReadCapability, PhysicalFeatureTransport},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

/// Mounted LowBatteryEffects slider bounds: DeviceInfo minLBE/maxLBE/StepLBE.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct LowBatteryRange {
    pub min: u8,
    pub max: u8,
    pub step: u8,
}
impl LowBatteryRange {
    pub fn contains(&self, value: u8) -> bool {
        (self.min..=self.max).contains(&value)
            && self.step > 0
            && (value - self.min).is_multiple_of(self.step)
    }
}

#[derive(Debug, Deserialize)]
pub struct Capability {
    product_id: u32,
    power_saving: bool,
    low_power: bool,
    low_battery: Option<LowBatteryRange>,
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
    pub fn low_battery(&self) -> Option<LowBatteryRange> {
        self.low_battery
    }
    pub fn receiver_session(
        &self,
        physical_product_id: u32,
        peer_product_id: u32,
    ) -> anyhow::Result<crate::receiver_session::ReceiverSession> {
        ensure!(
            self.dual && self.accepts(physical_product_id, Some(peer_product_id)),
            "Current power source has no matching DualLink peer"
        );
        crate::receiver_session::ReceiverSession::mouse_with_source(
            &self.transport,
            physical_product_id,
            peer_product_id,
        )
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
    CAPS.get_or_init(|| make_mouse_current_power_table())
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
                cap.low_battery.is_some_and(|range| range.contains(*value)),
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WriteFailure {
    /// Responses from the failed request; never a current device snapshot.
    pub progress: Report,
}

impl std::fmt::Display for WriteFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mouse power task did not complete")
    }
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    data: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Acknowledgement> {
    exchange_observing(
        device,
        cap,
        command,
        data,
        transaction,
        validate,
        &mut |_| {},
    )
}

fn exchange_observing(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    data: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    acknowledged: &mut dyn FnMut(&Acknowledgement),
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
    crate::feature_exchange::with_validated_exchange(
        device,
        crate::feature_exchange::with_protocol25_scope(
            crate::backend::ExchangeOptions::mutex_out(Duration::from_millis(
                cap.sleep_between_out_ms,
            )),
            &cap.source_class,
        ),
        validate,
        |device, lease| {
            let mut last = "No power response".to_owned();
            for out_attempt in 0..cap.max_retry_out {
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
                            let acknowledgement = Acknowledgement {
                                command,
                                transaction,
                                data: incoming[9..9 + size].to_vec(),
                            };
                            acknowledged(&acknowledgement);
                            return Ok(acknowledgement);
                        }
                        0 | 3 | 4 => {
                            last = format!("Power response status {}", incoming[1]);
                            break;
                        }
                        status => anyhow::bail!("Power command rejected with status {status}"),
                    }
                }
                if out_attempt + 1 < cap.max_retry_out {
                    lease.retry_release()?;
                }
            }
            anyhow::bail!("Current power source retries exhausted: {last}")
        },
    )
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
    let result = (|| -> anyhow::Result<()> {
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
                exchange_observing(
                    device,
                    cap,
                    [2, 7, 3],
                    &seconds.to_be_bytes(),
                    next()?,
                    &validate,
                    &mut |ack| {
                        report.acknowledgements.push(ack.clone());
                        report.setter_sent = true;
                    },
                )?;
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
                exchange_observing(
                    device,
                    cap,
                    [1, 7, 1],
                    &[raw],
                    next()?,
                    &validate,
                    &mut |ack| {
                        report.acknowledgements.push(ack.clone());
                        report.setter_sent = true;
                    },
                )?;
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
                    exchange_observing(
                        device,
                        cap,
                        [5, 7, 11],
                        &[0, u8::from(enabled), hi, lo, 20],
                        next()?,
                        &validate,
                        &mut |ack| {
                            report.acknowledgements.push(ack.clone());
                            report.setter_sent = true;
                        },
                    )?;
                }
            }
            Change::BatteryType(value) => {
                exchange_observing(
                    device,
                    cap,
                    [1, 7, 20],
                    &[value],
                    next()?,
                    &validate,
                    &mut |ack| {
                        report.acknowledgements.push(ack.clone());
                        report.setter_sent = true;
                    },
                )?;
            }
        }
        validate()
    })();
    if let Err(error) = result {
        let message = error.to_string();
        return Err(error
            .context(WriteFailure { progress: report })
            .context(message));
    }
    Ok(report)
}

#[cfg(test)]
#[path = "mouse_current_power_tests.rs"]
mod tests;

// Generated typed table: razer-device/mouse_current_power_data.
#[rustfmt::skip]
fn make_mouse_current_power_table() -> Vec<Capability> {
    vec![
        Capability{
            product_id: 89u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(90u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 89u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![89u32,90u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("89".into(),0u8),("90".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "89".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "90".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 98u32,
            power_saving: true,
            low_power: false,
            low_battery: Some(LowBatteryRange{min:3u8,max:10u8,step:1u8}),
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(98u32),
            ble_id: Some(97u32),
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 98u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![98u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("98".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "98".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 112u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(111u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 112u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![111u32,112u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("112".into(),0u8),("111".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "112".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "111".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 115u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(114u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 115u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![114u32,115u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("114".into(),0u8),("115".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "114".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "115".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 117u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: true,
            battery_type: false,
            dongle_id: Some(2308u32),
            ble_id: None,
            dual: false,
            mika: true,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 117u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![117u32,2308u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("2308".into(),0u8),("117".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "2308".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 20u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "117".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 122u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(123u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 122u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![122u32,123u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("122".into(),0u8),("123".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "122".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "123".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 124u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(125u32),
            ble_id: Some(142u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 124u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![124u32,125u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("124".into(),0u8),("125".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "124".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "125".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 128u32,
            power_saving: true,
            low_power: false,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(119u32),
            ble_id: Some(118u32),
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 128u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![119u32,128u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("128".into(),0u8),("119".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "128".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "119".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 131u32,
            power_saving: true,
            low_power: false,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(131u32),
            ble_id: Some(130u32),
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 131u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![131u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("131".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "131".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 134u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(136u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 134u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![134u32,136u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("136".into(),0u8),("134".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "136".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "134".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 143u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(144u32),
            ble_id: Some(146u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 143u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![143u32,144u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("144".into(),0u8),("143".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "144".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "143".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 148u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(148u32),
            ble_id: Some(149u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 148u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![148u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("148".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "148".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 154u32,
            power_saving: true,
            low_power: false,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(154u32),
            ble_id: Some(155u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 154u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![154u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("154".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "154".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 156u32,
            power_saving: true,
            low_power: false,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(156u32),
            ble_id: Some(157u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 156u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![156u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("156".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "156".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 158u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(159u32),
            ble_id: Some(0u32),
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 158u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![158u32,159u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("158".into(),0u8),("159".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "158".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "159".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 165u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(166u32),
            ble_id: None,
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 165u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![165u32,166u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("165".into(),0u8),("166".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "165".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "166".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 167u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(168u32),
            ble_id: Some(169u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 167u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![167u32,168u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("168".into(),0u8),("167".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "168".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "167".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 170u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(171u32),
            ble_id: Some(172u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 170u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![170u32,171u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("170".into(),0u8),("171".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "170".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "171".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 175u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(176u32),
            ble_id: Some(177u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 175u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![175u32,176u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("176".into(),0u8),("175".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "176".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "175".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 180u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(180u32),
            ble_id: Some(181u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 180u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![180u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("180".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "180".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 184u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: true,
            dongle_id: Some(184u32),
            ble_id: None,
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 184u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![184u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("184".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "184".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 185u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(185u32),
            ble_id: Some(186u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 185u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![185u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("185".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "185".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 190u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(191u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 190u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![190u32,191u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("190".into(),0u8),("191".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "190".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "191".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 192u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(193u32),
            ble_id: None,
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 192u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![192u32,193u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("192".into(),0u8),("193".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "192".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "193".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 194u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(195u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 194u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![194u32,195u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("194".into(),0u8),("195".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "194".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "195".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 196u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(197u32),
            ble_id: None,
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 196u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![196u32,197u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("196".into(),0u8),("197".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "196".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "197".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 199u32,
            power_saving: true,
            low_power: false,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(200u32),
            ble_id: Some(201u32),
            dual: false,
            mika: false,
            single_profile_seconds: true,
            always_read_power_saving: false,
            transport: DeviceReadCapability{
                product_id: 199u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![199u32,200u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("200".into(),0u8),("199".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "200".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "199".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 204u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(205u32),
            ble_id: Some(206u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 204u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![204u32,205u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("204".into(),0u8),("205".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "204".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "205".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 208u32,
            power_saving: true,
            low_power: false,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(209u32),
            ble_id: Some(210u32),
            dual: false,
            mika: false,
            single_profile_seconds: true,
            always_read_power_saving: false,
            transport: DeviceReadCapability{
                product_id: 208u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![208u32,209u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("208".into(),0u8),("209".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "208".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "209".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 211u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(212u32),
            ble_id: Some(213u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 211u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![211u32,212u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("211".into(),0u8),("212".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "211".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "212".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 214u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(215u32),
            ble_id: Some(216u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: false,
            transport: DeviceReadCapability{
                product_id: 214u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![214u32,215u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("214".into(),0u8),("215".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "214".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "215".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 218u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(219u32),
            ble_id: Some(220u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 218u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![218u32,219u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("218".into(),0u8),("219".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "218".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "219".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 221u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(221u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 221u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![221u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("221".into(),3u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "221".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 222u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(223u32),
            ble_id: None,
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 222u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![222u32,223u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("222".into(),0u8),("223".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "222".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "223".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 224u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: true,
            dongle_id: Some(224u32),
            ble_id: Some(225u32),
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 224u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![224u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("224".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "224".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 10u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 10u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 10u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 10u64
            }
        },
        Capability{
            product_id: 226u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(227u32),
            ble_id: Some(228u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 226u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![226u32,227u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("226".into(),3u8),("227".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "226".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "227".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 3u8,
                            report_id: 3u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 229u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(230u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: false,
            transport: DeviceReadCapability{
                product_id: 229u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![229u32,230u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("229".into(),3u8),("230".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "229".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "230".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 231u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(232u32),
            ble_id: Some(233u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 231u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![231u32,232u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("232".into(),3u8),("231".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "232".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 3u8,
                            report_id: 3u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "231".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 235u32,
            power_saving: true,
            low_power: false,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(234u32),
            ble_id: Some(236u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 235u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![234u32,235u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("234".into(),3u8),("235".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "234".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 3u8,
                            report_id: 3u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "235".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 239u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(240u32),
            ble_id: None,
            dual: false,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 239u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![239u32,240u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("240".into(),3u8),("239".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "240".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "239".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 2594u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: true,
            battery_type: false,
            dongle_id: Some(2593u32),
            ble_id: None,
            dual: false,
            mika: true,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 2594u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![2593u32,2594u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("2593".into(),0u8),("2594".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "2593".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 20u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "2594".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 45066u32,
            power_saving: true,
            low_power: true,
            low_battery: None,
            dim_lighting: false,
            battery_type: false,
            dongle_id: Some(45067u32),
            ble_id: Some(45068u32),
            dual: true,
            mika: false,
            single_profile_seconds: false,
            always_read_power_saving: true,
            transport: DeviceReadCapability{
                product_id: 45066u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![45066u32,45067u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("45066".into(),0u8),("45067".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "45066".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "45067".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        }
    ]
}

//! Current PID190 96204.be/Ne -> 84816.Y/Z and 85191.ot polling task.
//! The wired mouse and its ordinary receiver both use high-speed profile 1.
use crate::{
    backend::FeatureTransport,
    device_reads::{DeviceReadCapability, DeviceReadKind, ReadCommand},
    device_writes::{DeviceWriteAck, DeviceWriteResult, DeviceWriteSetting},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const PRODUCT_ID: u32 = 190;
pub const UI_RATES: [u32; 6] = [125, 500, 1000, 2000, 4000, 8000];
const GET: [u8; 3] = [2, 0, 0xc0];
const SET: [u8; 3] = [2, 0, 0x40];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Change {
    hz: u32,
    profile_guid: Option<String>,
}
impl Change {
    pub fn new(hz: u32) -> Self { Self { hz, profile_guid: None } }
    pub fn with_profile_guid(mut self, guid: String) -> Self { self.profile_guid = Some(guid); self }
    pub fn hz(&self) -> u32 { self.hz }
    pub fn profile_guid(&self) -> Option<&str> { self.profile_guid.as_deref() }
    pub fn validate(&self) -> anyhow::Result<()> {
        rate_code(self.hz)?;
        ensure!(self.profile_guid.as_ref().is_none_or(|guid| !guid.is_empty()), "Polling profile GUID is empty");
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Report {
    change: Change,
    runtime: DeviceWriteResult,
    profile_slots: Vec<crate::mouse_onboard::OnboardProfileAssociation>,
    slot_writes: Vec<SlotWrite>,
}
impl Report {
    pub fn change(&self) -> &Change { &self.change }
    pub fn runtime(&self) -> &DeviceWriteResult { &self.runtime }
    pub fn profile_slots(&self) -> &[crate::mouse_onboard::OnboardProfileAssociation] { &self.profile_slots }
    pub fn slot_writes(&self) -> &[SlotWrite] { &self.slot_writes }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SlotWrite {
    profile_id: u8,
    previous_hz: Option<u32>,
    acknowledgment: Option<DeviceWriteAck>,
}
impl SlotWrite {
    pub fn profile_id(&self) -> u8 { self.profile_id }
    pub fn previous_hz(&self) -> Option<u32> { self.previous_hz }
    pub fn acknowledgment(&self) -> Option<&DeviceWriteAck> { self.acknowledgment.as_ref() }
}

pub fn request(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    change: &Change,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    change.validate()?;
    let runtime = apply(device, cap, change.hz, &mut next, &validate)?;
    let mut report = Report { change: change.clone(), runtime, profile_slots: Vec::new(), slot_writes: Vec::new() };
    if let Some(guid) = change.profile_guid() {
        report.profile_slots = crate::mouse_onboard::read_profile_associations(device, cap, &mut next, &validate)?;
        report.slot_writes = synchronize_slots(device, cap, guid, &report.profile_slots, change.hz, &mut next, &validate)?;
    }
    validate()?;
    Ok(report)
}

/// 96571.rt selects the actual GUID's observed slots, then performs GET/SET.
/// A setter ACK confirms submission; this task has no OBM readback getter.
fn synchronize_slots(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    guid: &str,
    slots: &[crate::mouse_onboard::OnboardProfileAssociation],
    hz: u32,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<SlotWrite>> {
    let code = rate_code(hz)?;
    let mut writes = Vec::new();
    for slot in slots.iter().filter(|slot| slot.guid == guid) {
        validate()?;
        let getter = ReadCommand { name: DeviceReadKind::Polling, method: "getUSBHighSpeedPollingRate".into(), command: GET, payload: vec![slot.profile_id, 0], min_response_bytes: 2 };
        let previous = crate::device_query::read_device(device, cap, &getter, next()?, validate).ok();
        validate()?;
        let previous_hz = previous.and_then(|value| match value { crate::device_reads::DeviceReadValue::Polling { hz } => Some(hz), _ => None });
        let acknowledgment = if previous_hz == Some(hz) { None } else {
            let data = crate::feature_exchange::exchange(device, cap, SET, &[slot.profile_id, code], next()?, validate)?;
            Some(DeviceWriteAck { command: SET, data })
        };
        validate()?;
        writes.push(SlotWrite { profile_id: slot.profile_id, previous_hz, acknowledgment });
    }
    Ok(writes)
}

pub fn rate_code(hz: u32) -> anyhow::Result<u8> {
    Ok(match hz {
        125 => 64,
        250 => 32,
        500 => 16,
        1000 => 8,
        2000 => 4,
        4000 => 2,
        8000 => 1,
        _ => anyhow::bail!("Polling rate has no source high-speed code"),
    })
}

pub fn transport(physical_product_id: u32) -> anyhow::Result<DeviceReadCapability> {
    let mut cap = crate::mouse_current_dpi::capability(PRODUCT_ID)
        .context("Current polling physical factory is absent")?
        .transport()
        .for_physical_product(physical_product_id)?;
    ensure!(
        matches!(physical_product_id, 190 | 191) && cap.source_class == "rzDevice25",
        "Polling target differs from its current wired/ordinary receiver factory"
    );
    cap.queries = vec![ReadCommand {
        name: DeviceReadKind::Polling,
        method: "getUSBHighSpeedPollingRate".into(),
        command: GET,
        payload: vec![1, 0],
        min_response_bytes: 2,
    }];
    cap.polling_codes = [125, 250, 500, 1000, 2000, 4000, 8000]
        .into_iter()
        .map(|hz| (format!("RATE_{hz}Hz"), rate_code(hz).expect("source rate")))
        .collect();
    cap.polling_value_offset = Some(1);
    cap.polling_default_hz = Some(8000);
    cap.polling_physical_product_id = Some(physical_product_id);
    Ok(cap)
}

pub fn apply(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    hz: u32,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DeviceWriteResult> {
    ensure!(
        cap.product_id == PRODUCT_ID && cap.source_class == "rzDevice25",
        "Polling task requires the current PID190 sender"
    );
    let code = rate_code(hz)?;
    let setting = DeviceWriteSetting::Polling { hz };
    let getter = cap.queries.iter().find(|query| query.name == DeviceReadKind::Polling)
        .context("High-speed polling getter is absent")?;
    ensure!(getter.command == GET && getter.payload == [1, 0], "Polling getter profile differs");
    validate()?;
    // 96204.be catches a failed getter and returns zero to ot's comparison.
    // Missing observations remain explicit; a cancellation still stops SET.
    let previous = crate::device_query::read_device(device, cap, getter, next()?, &validate).ok();
    validate()?;
    if previous.as_ref().is_some_and(|value| setting.matches_value(value)) {
        return Ok(DeviceWriteResult {
            requested: setting,
            observed: previous.clone(),
            previous,
            acknowledged: None,
            changed: false,
            verified: true,
        });
    }
    let data = crate::feature_exchange::exchange(device, cap, SET, &[1, code], next()?, &validate)?;
    validate()?;
    let observed = crate::device_query::read_device(device, cap, getter, next()?, &validate)
        .context("Polling SET acknowledged, but independent GET failed")?;
    validate()?;
    ensure!(setting.matches_value(&observed), "Polling readback differs from the requested rate");
    Ok(DeviceWriteResult {
        requested: setting,
        previous,
        observed: Some(observed),
        acknowledged: Some(DeviceWriteAck { command: SET, data }),
        changed: true,
        verified: true,
    })
}

#[cfg(test)]
#[path = "mouse_current_polling_tests.rs"]
mod tests;

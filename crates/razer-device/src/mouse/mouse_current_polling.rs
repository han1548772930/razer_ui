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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    Rate { hz: u32 },
    Unknown { code: u8 },
}
impl Observation {
    pub fn hz(self) -> Option<u32> {
        match self {
            Self::Rate { hz } => Some(hz),
            Self::Unknown { .. } => None,
        }
    }
}

pub fn read(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Observation> {
    ensure!(
        cap.product_id == PRODUCT_ID && cap.source_class == "rzDevice25",
        "Polling read requires the current PID190 sender"
    );
    read_profile(device, cap, 1, &mut next, &validate)
}

fn read_profile(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    profile: u8,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Observation> {
    validate()?;
    let data =
        crate::feature_exchange::exchange(device, cap, GET, &[profile, 0], next()?, validate)?;
    validate()?;
    ensure!(
        data.len() >= 2 && data[0] == profile,
        "Polling GET profile or length differs"
    );
    Ok(match data[1] {
        64 => Observation::Rate { hz: 125 },
        32 => Observation::Rate { hz: 250 },
        16 => Observation::Rate { hz: 500 },
        8 => Observation::Rate { hz: 1000 },
        4 => Observation::Rate { hz: 2000 },
        2 => Observation::Rate { hz: 4000 },
        1 => Observation::Rate { hz: 8000 },
        code => Observation::Unknown { code },
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Change {
    hz: u32,
    profile_guid: Option<String>,
}
impl Change {
    pub fn new(hz: u32) -> Self {
        Self {
            hz,
            profile_guid: None,
        }
    }
    pub fn with_profile_guid(mut self, guid: String) -> Self {
        self.profile_guid = Some(guid);
        self
    }
    pub fn hz(&self) -> u32 {
        self.hz
    }
    pub fn profile_guid(&self) -> Option<&str> {
        self.profile_guid.as_deref()
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        rate_code(self.hz)?;
        ensure!(
            self.profile_guid
                .as_ref()
                .is_none_or(|guid| !guid.is_empty()),
            "Polling profile GUID is empty"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    change: Change,
    runtime: DeviceWriteResult,
    profile_slots: Vec<crate::mouse_onboard::OnboardProfileAssociation>,
    slot_writes: Vec<SlotWrite>,
}
impl Report {
    pub fn change(&self) -> &Change {
        &self.change
    }
    pub fn runtime(&self) -> &DeviceWriteResult {
        &self.runtime
    }
    pub fn profile_slots(&self) -> &[crate::mouse_onboard::OnboardProfileAssociation] {
        &self.profile_slots
    }
    pub fn slot_writes(&self) -> &[SlotWrite] {
        &self.slot_writes
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlotWrite {
    profile_id: u8,
    previous_hz: Option<u32>,
    acknowledgment: Option<DeviceWriteAck>,
}
impl SlotWrite {
    pub fn profile_id(&self) -> u8 {
        self.profile_id
    }
    pub fn previous_hz(&self) -> Option<u32> {
        self.previous_hz
    }
    pub fn acknowledgment(&self) -> Option<&DeviceWriteAck> {
        self.acknowledgment.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestFailure {
    /// Results belong to the failed request's identity, not a new connection.
    pub progress: Report,
}

impl std::fmt::Display for RequestFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mouse polling request did not complete")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlotWriteFailure {
    pub writes: Vec<SlotWrite>,
}

impl std::fmt::Display for SlotWriteFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mouse polling profile synchronization did not complete")
    }
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
    let mut report = Report {
        change: change.clone(),
        runtime,
        profile_slots: Vec::new(),
        slot_writes: Vec::new(),
    };
    let result = (|| -> anyhow::Result<()> {
        if let Some(guid) = change.profile_guid() {
            report.profile_slots =
                crate::mouse_onboard::read_profile_associations(device, cap, &mut next, &validate)?;
            let slot_writes = synchronize_slots(
                device,
                cap,
                guid,
                &report.profile_slots,
                change.hz,
                &mut next,
                &validate,
            );
            if let Err(error) = &slot_writes
                && let Some(progress) = error.downcast_ref::<SlotWriteFailure>()
            {
                report.slot_writes = progress.writes.clone();
            }
            report.slot_writes = slot_writes?;
        }
        validate()
    })();
    if let Err(error) = result {
        let message = error.to_string();
        return Err(error
            .context(RequestFailure { progress: report })
            .context(message));
    }
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
    let result = (|| -> anyhow::Result<()> {
        for slot in slots.iter().filter(|slot| slot.guid == guid) {
            validate()?;
            let previous = match read_profile(device, cap, slot.profile_id, next, validate) {
                Ok(previous) => Some(previous),
                Err(error) if crate::backend::exchange_interrupted(&error) => {
                    return Err(error);
                }
                Err(_) => None,
            };
            validate()?;
            let previous_hz = previous.and_then(Observation::hz);
            writes.push(SlotWrite {
                profile_id: slot.profile_id,
                previous_hz,
                acknowledgment: None,
            });
            if previous_hz != Some(hz) {
                let response = crate::feature_exchange::exchange(
                    device,
                    cap,
                    SET,
                    &[slot.profile_id, code],
                    next()?,
                    validate,
                );
                writes.last_mut().unwrap().acknowledgment =
                    crate::feature_exchange::acknowledged_reply(&response).map(|data| {
                        DeviceWriteAck {
                            command: SET,
                            data: data.to_vec(),
                        }
                    });
                response?;
            }
            validate()?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let message = error.to_string();
        return Err(error.context(SlotWriteFailure { writes }).context(message));
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
    cap.polling_default_hz = None;
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
    let getter = cap
        .queries
        .iter()
        .find(|query| query.name == DeviceReadKind::Polling)
        .context("High-speed polling getter is absent")?;
    ensure!(
        getter.command == GET && getter.payload == [1, 0],
        "Polling getter profile differs"
    );
    validate()?;
    // 96204.be catches a failed getter and returns zero to ot's comparison.
    // Missing observations remain explicit; a cancellation still stops SET.
    let previous = match read(device, cap, &mut next, &validate) {
        Ok(previous) => Some(previous),
        Err(error) if crate::backend::exchange_interrupted(&error) => return Err(error),
        Err(_) => None,
    }
    .and_then(Observation::hz)
    .map(|hz| crate::device_reads::DeviceReadValue::Polling { hz });
    validate()?;
    if previous
        .as_ref()
        .is_some_and(|value| setting.matches_value(value))
    {
        return Ok(DeviceWriteResult {
            requested: setting,
            observed: previous.clone(),
            previous,
            acknowledged: None,
            changed: false,
            verified: true,
        });
    }
    let mut progress = crate::device_writes::DeviceWriteFailure {
        requested: setting,
        previous,
        acknowledged: None,
        setter_reply: None,
        observed: None,
    };
    let result = (|| -> anyhow::Result<()> {
        let response =
            crate::feature_exchange::exchange(device, cap, SET, &[1, code], next()?, &validate);
        if let Some(data) = crate::feature_exchange::acknowledged_reply(&response) {
            progress.acknowledged = Some(DeviceWriteAck {
                command: SET,
                data: data.to_vec(),
            });
        }
        response?;
        validate()?;
        let reading = read(device, cap, &mut next, &validate)
            .context("Polling SET acknowledged, but independent GET failed")?;
        let observed = crate::device_reads::DeviceReadValue::Polling {
            hz: reading.hz().with_context(|| {
                format!("Polling SET acknowledged, but GET returned {reading:?}")
            })?,
        };
        let matches = progress.requested.matches_value(&observed);
        progress.observed = Some(observed);
        validate()?;
        ensure!(matches, "Polling readback differs from the requested rate");
        Ok(())
    })();
    if let Err(error) = result {
        let message = error.to_string();
        return Err(error.context(progress).context(message));
    }
    Ok(DeviceWriteResult {
        requested: progress.requested,
        previous: progress.previous,
        observed: progress.observed,
        acknowledged: progress.acknowledged,
        changed: true,
        verified: true,
    })
}

#[cfg(test)]
#[path = "mouse_current_polling_tests.rs"]
mod tests;

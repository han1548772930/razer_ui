//! Current controller MW modules 98773/87969, wrappers 33710/84392 and
//! mounted product injection. Evidence: `gamepad_triggers` in the maintained
//! gamepad-main-audit. Protocol bytes and timings do not depend on the OS.
use crate::backend::FeatureTransport;
use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerCapability {
    product_id: u32,
    report_id: u8,
    delay_ms: u64,
}

pub fn capability(product_id: u32) -> Option<TriggerCapability> {
    let (report_id, delay_ms) = match product_id {
        2629 | 2636 | 2650 | 4133 | 4144 => (4, 10),
        2647 | 2676 | 2684 => (10, 30),
        _ => return None,
    };
    Some(TriggerCapability {
        product_id,
        report_id,
        delay_ms,
    })
}

impl TriggerCapability {
    pub fn report_id(self) -> u8 {
        self.report_id
    }
    pub fn delay(self) -> Duration {
        Duration::from_millis(self.delay_ms)
    }
    pub fn accepts(self, physical_id: u32) -> bool {
        match self.product_id {
            2629 => physical_id == 2629,
            2636 => physical_id == 2636,
            2647 => matches!(physical_id, 2647 | 2649),
            2650 => physical_id == 2650,
            2676 => matches!(physical_id, 2676 | 2677 | 2680 | 2681),
            2684 => matches!(physical_id, 2684 | 2685),
            4133 => matches!(physical_id, 4133 | 4135 | 4136),
            4144 => matches!(physical_id, 4144 | 4146 | 4147),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalogTrigger {
    controller_id: u8,
    start_range: u8,
    end_range: u8,
    is_active: bool,
}
impl AnalogTrigger {
    pub fn new(
        controller_id: u8,
        start_range: u8,
        end_range: u8,
        is_active: bool,
    ) -> anyhow::Result<Self> {
        ensure!(
            matches!(controller_id, 1 | 2),
            "Invalid controller trigger ID"
        );
        ensure!(
            start_range < end_range && end_range <= 100,
            "Invalid analog trigger range"
        );
        Ok(Self {
            controller_id,
            start_range,
            end_range,
            is_active,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DigitalTrigger {
    controller_id: u8,
    actuation_point: u8,
    is_rapid_trigger: bool,
    is_active: bool,
}
impl DigitalTrigger {
    pub fn new(
        controller_id: u8,
        actuation_point: u8,
        is_rapid_trigger: bool,
        is_active: bool,
    ) -> anyhow::Result<Self> {
        ensure!(
            matches!(controller_id, 1 | 2),
            "Invalid controller trigger ID"
        );
        // Source default profiles contain zero for the inactive digital branch.
        ensure!(actuation_point <= 100, "Invalid digital trigger point");
        Ok(Self {
            controller_id,
            actuation_point,
            is_rapid_trigger,
            is_active,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TriggerOperation {
    Read {
        profile_slot: u8,
    },
    Analog {
        profile_slot: u8,
        settings: Vec<AnalogTrigger>,
    },
    Digital {
        profile_slot: u8,
        settings: Vec<DigitalTrigger>,
    },
    /// The same current controller collection owns the thumbstick commands.
    /// Keep this on its existing ordered/cancelable operation seam.
    ReadThumbsticks {
        profile_slot: u8,
    },
    ThumbstickDeadzone {
        profile_slot: u8,
        settings: Vec<ThumbstickValue>,
        prevent_double_deadzone: bool,
    },
    ThumbstickSensitivity {
        profile_slot: u8,
        settings: Vec<ThumbstickValue>,
    },
    Circularity {
        profile_slot: u8,
        circular: bool,
    },
}
impl TriggerOperation {
    pub fn is_read(&self) -> bool {
        matches!(self, Self::Read { .. } | Self::ReadThumbsticks { .. })
    }
    pub fn is_thumbstick(&self) -> bool {
        matches!(
            self,
            Self::ReadThumbsticks { .. }
                | Self::ThumbstickDeadzone { .. }
                | Self::ThumbstickSensitivity { .. }
                | Self::Circularity { .. }
        )
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        let validate_ids = |ids: Vec<u8>| -> anyhow::Result<()> {
            ensure!(
                ids.len() <= 2 && ids.iter().all(|id| matches!(id, 1 | 2)),
                "Invalid trigger list"
            );
            ensure!(
                ids.len() < 2 || ids[0] != ids[1],
                "Duplicate trigger controller ID"
            );
            Ok(())
        };
        match self {
            Self::Read { .. } | Self::ReadThumbsticks { .. } | Self::Circularity { .. } => {}
            Self::ThumbstickDeadzone { settings, .. }
            | Self::ThumbstickSensitivity { settings, .. } => {
                ensure!(!settings.is_empty(), "Empty controller thumbstick write");
                validate_ids(settings.iter().map(|s| s.controller_id).collect())?;
                for setting in settings {
                    ThumbstickValue::new(setting.controller_id, setting.value)?;
                }
            }
            Self::Analog { settings, .. } => {
                validate_ids(settings.iter().map(|s| s.controller_id).collect())?;
                ensure!(
                    settings.iter().any(|s| s.is_active),
                    "No active analog trigger write"
                );
                for s in settings {
                    AnalogTrigger::new(s.controller_id, s.start_range, s.end_range, s.is_active)?;
                }
            }
            Self::Digital { settings, .. } => {
                validate_ids(settings.iter().map(|s| s.controller_id).collect())?;
                ensure!(
                    settings.iter().any(|s| s.is_active),
                    "No active digital trigger write"
                );
                for s in settings {
                    DigitalTrigger::new(
                        s.controller_id,
                        s.actuation_point,
                        s.is_rapid_trigger,
                        s.is_active,
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThumbstickValue {
    controller_id: u8,
    value: u16,
}
impl ThumbstickValue {
    pub fn new(controller_id: u8, value: u16) -> anyhow::Result<Self> {
        ensure!(
            matches!(controller_id, 1 | 2),
            "Invalid controller thumbstick ID"
        );
        ensure!(value <= 100, "Invalid controller thumbstick percentage");
        Ok(Self {
            controller_id,
            value,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerProfile {
    #[serde(default)]
    analog_actuations: Vec<AnalogTrigger>,
    #[serde(default)]
    digital_actuations: Vec<DigitalTrigger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thumbsticks: Option<ThumbstickProfile>,
}
impl TriggerProfile {
    pub fn analog_actuations(&self) -> &[AnalogTrigger] {
        &self.analog_actuations
    }
    pub fn digital_actuations(&self) -> &[DigitalTrigger] {
        &self.digital_actuations
    }
    pub fn thumbsticks(&self) -> Option<&ThumbstickProfile> {
        self.thumbsticks.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbstickProfile {
    deadzones: Vec<ThumbstickValue>,
    prevent_double_deadzone: bool,
    sensitivity_values: Vec<ThumbstickValue>,
    circular: bool,
}
impl ThumbstickProfile {
    pub fn deadzones(&self) -> &[ThumbstickValue] {
        &self.deadzones
    }
    pub fn prevent_double_deadzone(&self) -> bool {
        self.prevent_double_deadzone
    }
    pub fn sensitivity_values(&self) -> &[ThumbstickValue] {
        &self.sensitivity_values
    }
    pub fn circular(&self) -> bool {
        self.circular
    }
}

#[derive(Clone, Copy, Debug)]
enum Command {
    GetAnalog(u8, u8),
    SetAnalog(u8, u8, u8, u8),
    GetMode(u8, u8),
    SetMode(u8, u8, u8),
    GetDigital(u8, u8),
    SetDigital(u8, u8, u8, bool),
    GetZones(u8, u8),
    SetZones(u8, u8, u16, bool),
    GetSensitivity(u8, u8),
    SetSensitivity(u8, u8, u8),
    GetCircularity(u8, u8),
    SetCircularity(u8, u8, bool),
}
impl Command {
    fn header(self) -> [u8; 3] {
        match self {
            Self::GetAnalog(..) => [4, 12, 147],
            Self::SetAnalog(..) => [4, 12, 19],
            Self::GetMode(..) => [3, 12, 149],
            Self::SetMode(..) => [3, 12, 21],
            Self::GetDigital(..) => [4, 12, 150],
            Self::SetDigital(..) => [4, 12, 22],
            Self::GetZones(..) => [9, 12, 133],
            Self::SetZones(..) => [9, 12, 5],
            Self::GetSensitivity(..) => [3, 12, 130],
            Self::SetSensitivity(..) => [3, 12, 2],
            Self::GetCircularity(..) => [3, 12, 146],
            Self::SetCircularity(..) => [3, 12, 18],
        }
    }
    fn data(self) -> [u8; 9] {
        match self {
            Self::GetAnalog(slot, side)
            | Self::GetMode(slot, side)
            | Self::GetDigital(slot, side)
            | Self::GetZones(slot, side)
            | Self::GetSensitivity(slot, side)
            | Self::GetCircularity(slot, side) => [slot, side, 0, 0, 0, 0, 0, 0, 0],
            Self::SetAnalog(slot, side, start, end) => [slot, side, start, end, 0, 0, 0, 0, 0],
            Self::SetMode(slot, side, mode) | Self::SetSensitivity(slot, side, mode) => {
                [slot, side, mode, 0, 0, 0, 0, 0, 0]
            }
            Self::SetDigital(slot, side, point, rapid) => {
                [slot, side, point, u8::from(rapid), 0, 0, 0, 0, 0]
            }
            Self::SetCircularity(slot, side, circular) => {
                [slot, side, u8::from(circular), 0, 0, 0, 0, 0, 0]
            }
            Self::SetZones(slot, side, value, prevent) => {
                let [high, low] = value.to_be_bytes();
                [slot, side, high, low, 0, 0, 0, 0, u8::from(prevent)]
            }
        }
    }
    fn report(self, report_id: u8, transaction: u8) -> [u8; 91] {
        let mut report = [0; 91];
        report[0] = report_id;
        report[2] = transaction;
        let header = self.header();
        report[6..9].copy_from_slice(&header);
        report[9..9 + usize::from(header[0])]
            .copy_from_slice(&self.data()[..usize::from(header[0])]);
        report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
        report
    }
}

/// The caller owns an exclusive, identity-checked collection and supplies a
/// cancelable wait. A write result is firmware acknowledgement, never readback.
pub struct TriggerProtocol<
    'a,
    T: FeatureTransport + ?Sized,
    W: FnMut(Duration) -> anyhow::Result<()>,
> {
    transport: &'a T,
    capability: TriggerCapability,
    wait: W,
    transaction: u8,
}
impl<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
    TriggerProtocol<'a, T, W>
{
    pub fn new(transport: &'a T, capability: TriggerCapability, wait: W) -> Self {
        Self {
            transport,
            capability,
            wait,
            transaction: 0,
        }
    }
    fn exchange(&mut self, command: Command) -> anyhow::Result<Vec<u8>> {
        let transaction = self.transaction;
        self.transaction = (transaction + 1) % 31;
        let request = command.report(self.capability.report_id, transaction);
        let mut error = String::from("No trigger reply");
        for _ in 0..20 {
            (self.wait)(self.capability.delay())?;
            if let Err(e) = self.transport.send_feature(&request) {
                error = e.to_string();
                continue;
            }
            (self.wait)(self.capability.delay())?;
            for retry in 0..10 {
                if retry != 0 {
                    (self.wait)(self.capability.delay())?;
                }
                let mut response = [0; 91];
                response[0] = self.capability.report_id;
                let count = match self.transport.get_feature(&mut response) {
                    Ok(count) => count,
                    Err(e) => {
                        error = e.to_string();
                        break;
                    }
                };
                ensure!(
                    count == 91 && response[0] == self.capability.report_id,
                    "Truncated or wrong controller Feature report"
                );
                if response[2] != transaction || response[7..9] != request[7..9] {
                    error = "Stale controller response".into();
                    break;
                }
                match response[1] {
                    0 | 3 | 4 => {
                        error = format!("Controller retry status {}", response[1]);
                        break;
                    }
                    1 => error = "Controller busy".into(),
                    2 => {
                        let size = usize::from(response[6]);
                        ensure!(size <= 80, "Invalid controller response payload length");
                        return Ok(response[9..9 + size].to_vec());
                    }
                    status => bail!("Controller trigger command rejected with status {status}"),
                }
            }
        }
        bail!("Controller trigger retries exhausted: {error}")
    }
    pub fn execute(
        &mut self,
        operation: &TriggerOperation,
    ) -> anyhow::Result<Option<TriggerProfile>> {
        operation.validate()?;
        match operation {
            TriggerOperation::Read { profile_slot } => {
                let mut analog_actuations = vec![];
                let mut digital_actuations = vec![];
                // Source runs the analog wrapper (range + mode for each side),
                // then the digital wrapper. Each wrapper requires four replies.
                for side in [1, 2] {
                    let range = self.exchange(Command::GetAnalog(*profile_slot, side))?;
                    let mode = self.exchange(Command::GetMode(*profile_slot, side))?;
                    ensure!(
                        range.len() >= 4 && mode.len() >= 3,
                        "Truncated analog trigger data"
                    );
                    analog_actuations.push(AnalogTrigger::new(
                        side,
                        range[2],
                        range[3],
                        mode[2] == 0,
                    )?);
                }
                for side in [1, 2] {
                    let digital = self.exchange(Command::GetDigital(*profile_slot, side))?;
                    let mode = self.exchange(Command::GetMode(*profile_slot, side))?;
                    ensure!(
                        digital.len() >= 4 && mode.len() >= 3,
                        "Truncated digital trigger data"
                    );
                    digital_actuations.push(DigitalTrigger::new(
                        side,
                        digital[2],
                        digital[3] != 0,
                        mode[2] == 1,
                    )?);
                }
                Ok(Some(TriggerProfile {
                    analog_actuations,
                    digital_actuations,
                    thumbsticks: None,
                }))
            }
            TriggerOperation::ReadThumbsticks { profile_slot } => {
                // 69005 reads circularity on side1 only. 91564/59785 require
                // both side replies and share the left prevention flag.
                let circularity = self.exchange(Command::GetCircularity(*profile_slot, 1))?;
                ensure!(circularity.len() >= 3, "Truncated thumbstick circularity");
                let mut deadzones = Vec::new();
                let mut prevent_double_deadzone = false;
                for side in [1, 2] {
                    let zones = self.exchange(Command::GetZones(*profile_slot, side))?;
                    ensure!(zones.len() >= 9, "Truncated thumbstick deadzone");
                    if side == 1 {
                        prevent_double_deadzone = zones[8] != 0;
                    }
                    deadzones.push(ThumbstickValue::new(
                        side,
                        u16::from_be_bytes([zones[2], zones[3]]),
                    )?);
                }
                let mut sensitivity_values = Vec::new();
                for side in [1, 2] {
                    let sensitivity =
                        self.exchange(Command::GetSensitivity(*profile_slot, side))?;
                    ensure!(sensitivity.len() >= 3, "Truncated thumbstick sensitivity");
                    sensitivity_values.push(ThumbstickValue::new(side, u16::from(sensitivity[2]))?);
                }
                Ok(Some(TriggerProfile {
                    analog_actuations: Vec::new(),
                    digital_actuations: Vec::new(),
                    thumbsticks: Some(ThumbstickProfile {
                        deadzones,
                        prevent_double_deadzone,
                        sensitivity_values,
                        circular: circularity[2] != 0,
                    }),
                }))
            }
            TriggerOperation::ThumbstickDeadzone {
                profile_slot,
                settings,
                prevent_double_deadzone,
            } => {
                for setting in settings {
                    self.exchange(Command::SetZones(
                        *profile_slot,
                        setting.controller_id,
                        setting.value,
                        *prevent_double_deadzone,
                    ))?;
                }
                Ok(None)
            }
            TriggerOperation::ThumbstickSensitivity {
                profile_slot,
                settings,
            } => {
                for setting in settings {
                    self.exchange(Command::SetSensitivity(
                        *profile_slot,
                        setting.controller_id,
                        setting.value as u8,
                    ))?;
                }
                Ok(None)
            }
            TriggerOperation::Circularity {
                profile_slot,
                circular,
            } => {
                for side in [1, 2] {
                    self.exchange(Command::SetCircularity(*profile_slot, side, *circular))?;
                }
                Ok(None)
            }
            TriggerOperation::Analog {
                profile_slot,
                settings,
            } => {
                for s in settings.iter().filter(|s| s.is_active) {
                    self.exchange(Command::SetMode(*profile_slot, s.controller_id, 0))?;
                    self.exchange(Command::SetAnalog(
                        *profile_slot,
                        s.controller_id,
                        s.start_range,
                        s.end_range,
                    ))?;
                }
                Ok(None)
            }
            TriggerOperation::Digital {
                profile_slot,
                settings,
            } => {
                for s in settings.iter().filter(|s| s.is_active) {
                    self.exchange(Command::SetMode(*profile_slot, s.controller_id, 1))?;
                    self.exchange(Command::SetDigital(
                        *profile_slot,
                        s.controller_id,
                        s.actuation_point,
                        s.is_rapid_trigger,
                    ))?;
                }
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
#[path = "controller_triggers_tests.rs"]
mod tests;

//! Current 2636 Sage and 2676 SagePC Power wrappers and task runner.
//! The page supplies minutes; the independent confirmation reads raw seconds.
use crate::backend::FeatureTransport;
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepCapability {
    product_id: u32,
}

pub fn capability(product_id: u32) -> Option<SleepCapability> {
    matches!(product_id, 2636 | 2676).then_some(SleepCapability { product_id })
}

impl SleepCapability {
    pub fn report_id(self) -> u8 {
        if self.product_id == 2636 { 4 } else { 10 }
    }
    pub fn delay(self) -> Duration {
        Duration::from_millis(if self.product_id == 2636 { 10 } else { 30 })
    }
    pub fn accepts(self, physical_id: u32) -> bool {
        match self.product_id {
            2636 => physical_id == 2636,
            2676 => matches!(physical_id, 2676 | 2677 | 2680 | 2681),
            _ => false,
        }
    }
    fn encode(self, seconds: u16) -> [u8; 2] {
        if self.product_id == 2636 {
            seconds.to_le_bytes()
        } else {
            seconds.to_be_bytes()
        }
    }
    fn decode(self, bytes: [u8; 2]) -> u16 {
        if self.product_id == 2636 {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", try_from = "PowerSavingFields")]
#[non_exhaustive]
pub struct PowerSaving {
    /// The mounted current Power component and its event both use minutes.
    pub value: u16,
    pub is_enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PowerSavingFields {
    value: u16,
    is_enabled: bool,
}
impl TryFrom<PowerSavingFields> for PowerSaving {
    type Error = String;
    fn try_from(fields: PowerSavingFields) -> Result<Self, Self::Error> {
        Self::new(fields.value, fields.is_enabled).map_err(|error| error.to_string())
    }
}
impl PowerSaving {
    pub fn new(value: u16, is_enabled: bool) -> anyhow::Result<Self> {
        let setting = Self { value, is_enabled };
        setting.seconds()?;
        Ok(setting)
    }
    pub fn seconds(self) -> anyhow::Result<u16> {
        self.value.checked_mul(60).ok_or_else(|| {
            anyhow::anyhow!("Controller sleep minutes exceed the two-byte seconds field")
        })
    }
    pub fn minutes(self) -> u16 {
        self.value
    }
    pub fn is_enabled(self) -> bool {
        self.is_enabled
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SleepOperation {
    Read,
    Set { power_saving: PowerSaving },
}
impl SleepOperation {
    pub fn is_read(&self) -> bool {
        matches!(self, Self::Read)
    }
    pub fn setting(&self) -> Option<PowerSaving> {
        match self {
            Self::Read => None,
            Self::Set { power_saving } => Some(*power_saving),
        }
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Self::Set { power_saving } = self {
            power_saving.seconds()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct SleepObservation {
    pub seconds: u16,
}
impl SleepObservation {
    pub fn from_seconds(seconds: u16) -> Self {
        Self { seconds }
    }
    pub fn seconds(self) -> u16 {
        self.seconds
    }
    /// Positive JS Math.round(seconds / 60), including the half-minute boundary.
    pub fn power_saving(self) -> PowerSaving {
        let value = (u32::from(self.seconds) + 30) / 60;
        PowerSaving {
            value: value as u16,
            is_enabled: value != 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SleepWriteStatus {
    NotRequested,
    NotAttempted,
    Acknowledged,
    Failed { error: String },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SleepReadStatus {
    Unobserved,
    Observed { observation: SleepObservation },
    Failed { error: String },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SleepCompletion {
    Observed,
    Confirmed,
    Mismatch,
    Failed,
    CanceledBeforeOperation,
    CanceledAfterSet,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct SleepResult {
    pub write: SleepWriteStatus,
    pub read: SleepReadStatus,
    pub completion: SleepCompletion,
}
impl SleepResult {
    pub fn new(
        operation: &SleepOperation,
        write: SleepWriteStatus,
        read: SleepReadStatus,
        completion: SleepCompletion,
    ) -> anyhow::Result<Self> {
        let result = Self {
            write,
            read,
            completion,
        };
        result.validate_for(operation)?;
        Ok(result)
    }
    pub fn observation(&self) -> Option<SleepObservation> {
        match self.read {
            SleepReadStatus::Observed { observation } => Some(observation),
            _ => None,
        }
    }
    pub fn validate_for(&self, operation: &SleepOperation) -> anyhow::Result<()> {
        operation.validate()?;
        let write_requested = !operation.is_read();
        ensure!(
            matches!(self.write, SleepWriteStatus::NotRequested) != write_requested,
            "Controller sleep write status differs from the requested operation"
        );
        if let SleepWriteStatus::Failed { error } = &self.write {
            ensure!(
                !error.is_empty(),
                "Controller sleep setter failure omits its error"
            );
        }
        if let SleepReadStatus::Failed { error } = &self.read {
            ensure!(
                !error.is_empty(),
                "Controller sleep getter failure omits its error"
            );
        }
        if matches!(self.write, SleepWriteStatus::NotAttempted) {
            ensure!(
                self.completion == SleepCompletion::CanceledBeforeOperation,
                "Controller sleep setter was not attempted before a getter"
            );
        }
        match self.completion {
            SleepCompletion::Observed => ensure!(
                operation.is_read()
                    && self.write == SleepWriteStatus::NotRequested
                    && self.observation().is_some(),
                "Controller sleep read has no real observation"
            ),
            SleepCompletion::Confirmed | SleepCompletion::Mismatch => {
                let expected = operation
                    .setting()
                    .ok_or_else(|| anyhow::anyhow!("Controller sleep read cannot confirm a write"))?
                    .seconds()?;
                let observed = self.observation().ok_or_else(|| {
                    anyhow::anyhow!("Controller sleep confirmation has no readback")
                })?;
                ensure!(
                    matches!(
                        self.write,
                        SleepWriteStatus::Acknowledged | SleepWriteStatus::Failed { .. }
                    ) && (observed.seconds == expected)
                        == (self.completion == SleepCompletion::Confirmed),
                    "Controller sleep confirmation differs from the raw seconds readback"
                );
            }
            SleepCompletion::Failed => ensure!(
                matches!(self.read, SleepReadStatus::Failed { .. })
                    && (operation.is_read()
                        || matches!(
                            self.write,
                            SleepWriteStatus::Acknowledged | SleepWriteStatus::Failed { .. }
                        )),
                "Controller sleep failed result omits the getter failure"
            ),
            SleepCompletion::CanceledBeforeOperation => ensure!(
                self.write
                    == (if operation.is_read() {
                        SleepWriteStatus::NotRequested
                    } else {
                        SleepWriteStatus::NotAttempted
                    })
                    && self.read == SleepReadStatus::Unobserved,
                "Controller sleep canceled-before-operation includes a transfer"
            ),
            SleepCompletion::CanceledAfterSet => ensure!(
                write_requested
                    && matches!(
                        self.write,
                        SleepWriteStatus::Acknowledged | SleepWriteStatus::Failed { .. }
                    )
                    && self.read == SleepReadStatus::Unobserved,
                "Controller sleep canceled-after-set has inconsistent transfers"
            ),
        }
        Ok(())
    }
}

pub struct SleepProtocol<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
{
    transport: &'a T,
    capability: SleepCapability,
    wait: W,
    transaction: u8,
}
impl<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
    SleepProtocol<'a, T, W>
{
    pub fn new(transport: &'a T, capability: SleepCapability, wait: W) -> Self {
        Self {
            transport,
            capability,
            wait,
            transaction: 0,
        }
    }
    fn exchange(&mut self, command: u8, data: &[u8; 2]) -> anyhow::Result<Vec<u8>> {
        let transaction = self.transaction;
        self.transaction = (transaction + 1) % 31;
        let request = crate::controller_feature::report(
            self.capability.report_id(),
            transaction,
            [2, 7, command],
            data,
        )?;
        crate::controller_feature::exchange(
            self.transport,
            &request,
            self.capability.delay(),
            &mut self.wait,
        )
    }
    fn read(&mut self) -> anyhow::Result<SleepObservation> {
        let data = self.exchange(0x83, &[0, 0])?;
        let bytes: [u8; 2] = data.try_into().map_err(|data: Vec<u8>| {
            anyhow::anyhow!(
                "Controller sleep getter returned {} bytes, expected two",
                data.len()
            )
        })?;
        Ok(SleepObservation {
            seconds: self.capability.decode(bytes),
        })
    }
    pub fn execute(&mut self, operation: &SleepOperation) -> anyhow::Result<SleepResult> {
        self.execute_with_cancel(operation, || false)
    }
    pub fn execute_with_cancel(
        &mut self,
        operation: &SleepOperation,
        mut canceled: impl FnMut() -> bool,
    ) -> anyhow::Result<SleepResult> {
        operation.validate()?;
        let mut result = SleepResult {
            write: if operation.is_read() {
                SleepWriteStatus::NotRequested
            } else {
                SleepWriteStatus::NotAttempted
            },
            read: SleepReadStatus::Unobserved,
            completion: SleepCompletion::CanceledBeforeOperation,
        };
        if canceled() {
            return Ok(result);
        }
        let expected = if let Some(setting) = operation.setting() {
            let seconds = setting.seconds()?;
            // The current override catches setter exceptions; the runner still
            // performs its independent getter and compares raw seconds.
            result.write = match self.exchange(3, &self.capability.encode(seconds)) {
                Ok(_) => SleepWriteStatus::Acknowledged,
                Err(error) => SleepWriteStatus::Failed {
                    error: format!("{error:#}"),
                },
            };
            if canceled() {
                result.completion = SleepCompletion::CanceledAfterSet;
                return Ok(result);
            }
            Some(seconds)
        } else {
            None
        };
        match self.read() {
            Ok(observation) => {
                result.read = SleepReadStatus::Observed { observation };
                result.completion = match expected {
                    Some(seconds) if seconds == observation.seconds => SleepCompletion::Confirmed,
                    Some(_) => SleepCompletion::Mismatch,
                    None => SleepCompletion::Observed,
                };
            }
            Err(error) => {
                result.read = SleepReadStatus::Failed {
                    error: format!("{error:#}"),
                };
                result.completion = SleepCompletion::Failed;
            }
        }
        // The official runner has no abort checkpoint after the getter.
        result.validate_for(operation)?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "controller_sleep_tests.rs"]
mod tests;

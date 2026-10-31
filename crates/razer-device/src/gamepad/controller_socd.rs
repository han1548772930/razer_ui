//! Mounted 2676 generic SOCD setter. Its SUCCESS response acknowledges a SET;
//! it is separate from the connection phase's independently read option.
use crate::{backend::FeatureTransport, controller_initialization::SocdMode};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocdOperation {
    /// Observed first obmSlotId, retained separately from the Fk wire encoding.
    pub slot: u8,
    pub mode: SocdMode,
}
impl SocdOperation {
    pub fn wire_slot(self) -> u8 {
        crate::controller_profile::wire_slot(self.slot)
    }
    pub fn validate(self, product_id: u32) -> anyhow::Result<()> {
        ensure!(
            product_id == 2676,
            "Controller SOCD feature is not mounted on this product"
        );
        Ok(())
    }
    fn option(self) -> u8 {
        match self.mode {
            SocdMode::Neutral => 0,
            SocdMode::Last => 1,
            SocdMode::First => 2,
            SocdMode::None => 3,
            SocdMode::AbsoluteUp => 4,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SocdWriteStatus {
    NotAttempted,
    Acknowledged { response_payload: Vec<u8> },
    Failed { error: String },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocdResult {
    pub write: SocdWriteStatus,
    pub canceled: bool,
}
impl SocdResult {
    pub fn validate(&self) -> anyhow::Result<()> {
        match &self.write {
            SocdWriteStatus::NotAttempted => {
                ensure!(self.canceled, "Unattempted SOCD SET is not canceled")
            }
            SocdWriteStatus::Acknowledged { response_payload } => ensure!(
                response_payload.len() <= 80,
                "SOCD acknowledgement exceeds Protocol25 payload"
            ),
            SocdWriteStatus::Failed { error } => {
                ensure!(!error.is_empty(), "SOCD failure omits the transport error")
            }
        }
        Ok(())
    }
    pub fn acknowledged(&self) -> bool {
        !self.canceled && matches!(self.write, SocdWriteStatus::Acknowledged { .. })
    }
}

pub struct SocdProtocol<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
{
    transport: &'a T,
    wait: W,
    transaction: u8,
}
impl<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
    SocdProtocol<'a, T, W>
{
    pub fn new(transport: &'a T, wait: W) -> Self {
        Self {
            transport,
            wait,
            transaction: 0,
        }
    }
    pub fn execute(
        &mut self,
        product_id: u32,
        operation: SocdOperation,
        mut canceled: impl FnMut() -> bool,
    ) -> anyhow::Result<SocdResult> {
        operation.validate(product_id)?;
        if canceled() {
            return Ok(SocdResult {
                write: SocdWriteStatus::NotAttempted,
                canceled: true,
            });
        }
        let request = crate::controller_feature::report(
            10,
            self.transaction,
            [2, 12, 27],
            &[operation.wire_slot(), operation.option()],
        )?;
        self.transaction = (self.transaction + 1) % 31;
        let write = match crate::controller_feature::exchange(
            self.transport,
            &request,
            Duration::from_millis(30),
            &mut self.wait,
        ) {
            Ok(response_payload) => SocdWriteStatus::Acknowledged { response_payload },
            Err(error) if crate::controller_feature::terminal_io_failure(&error) => {
                return Err(error);
            }
            Err(error) => SocdWriteStatus::Failed {
                error: format!("{error:#}"),
            },
        };
        // Module 84470 checks SUCCESS only. Neither a GET nor option/slot
        // equality is part of this setter's acknowledgement contract.
        let result = SocdResult {
            write,
            canceled: canceled(),
        };
        result.validate()?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "controller_socd_tests.rs"]
mod tests;

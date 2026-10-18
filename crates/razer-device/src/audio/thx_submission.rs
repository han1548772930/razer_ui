//! Typed local THX jobs; drafts, acknowledgements and observations are separate.
use crate::thx_blackshark_firmware::{
    BlackSharkFirmwareCommandProgress, BlackSharkLocalEqRestoreProgress,
};
use crate::thx_v4::{
    SpatialOutcome, ThxRenderSetupOutcome, ThxRenderSetupTrigger, ThxRenderShutdownOutcome,
    capability,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThxSubmissionScope {
    pub product_id: u32,
    pub container_id: String,
    pub serial_number: String,
    pub profile_id: String,
    pub generation: u64,
}
impl ThxSubmissionScope {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            capability(self.product_id).is_some(),
            "THX owner lacks a current source audit"
        );
        ensure!(
            !self.container_id.is_empty() && !self.container_id.starts_with("hid-collection:"),
            "THX needs an observed OS ContainerId"
        );
        for value in [&self.container_id, &self.serial_number, &self.profile_id] {
            ensure!(
                !value.contains('\0') && value.len() <= 4096,
                "Invalid THX scope text"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThxSubmissionPlan {
    /// Render core setup only. Replay/capture/workaround are separate chains.
    SetupRender {
        trigger: ThxRenderSetupTrigger,
    },
    /// Only on an existing render owner. This is distinct from cancellation,
    /// disconnect and releasing a one-off query's Rust binding.
    ShutdownRender,
    ReadSpatial,
    ReadFirmwareVersion,
    FactoryReset1398,
    ReadRollerMapping,
    SetRollerMappings {
        mappings: Vec<Value>,
    },
    SetSpatial {
        enabled: bool,
        presets: Vec<Value>,
        timer_tick: u64,
    },
}
impl ThxSubmissionPlan {
    pub fn is_direct_hid(&self) -> bool {
        self.is_roller_mapping()
            || matches!(self, Self::ReadFirmwareVersion | Self::FactoryReset1398)
    }
    pub fn is_roller_mapping(&self) -> bool {
        matches!(
            self,
            Self::ReadRollerMapping | Self::SetRollerMappings { .. }
        )
    }
    pub fn validate(&self) -> Result<()> {
        if let Self::SetRollerMappings { mappings } = self {
            crate::blackshark_roller_mapping::validate_mappings(mappings)?;
        }
        if let Self::SetSpatial { presets, .. } = self {
            // Rust resource boundary, independent of source preset count.
            ensure!(
                presets.len() <= 256,
                "THX preset count exceeds USB index space"
            );
            ensure!(
                serde_json::to_vec(presets)?.len() <= 512 * 1024,
                "THX preset request too large"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThxFirmwareProgress {
    Command(BlackSharkFirmwareCommandProgress),
    LocalEq(BlackSharkLocalEqRestoreProgress),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThxSubmissionProgress {
    pub scope: ThxSubmissionScope,
    pub plan: ThxSubmissionPlan,
    #[serde(default)]
    pub render_setup: Option<ThxRenderSetupOutcome>,
    #[serde(default)]
    pub render_shutdown: Option<ThxRenderShutdownOutcome>,
    #[serde(default)]
    pub roller_mapping: Option<crate::blackshark_roller_mapping::RollerMappingProgress>,
    pub initial_observation: Option<SpatialOutcome>,
    pub outcome: Option<SpatialOutcome>,
    pub firmware: Vec<ThxFirmwareProgress>,
    pub error: Option<String>,
    pub cleanup_error: Option<String>,
}
impl ThxSubmissionProgress {
    pub fn observed_firmware_version(&self) -> Option<String> {
        self.firmware
            .iter()
            .rev()
            .find_map(|progress| match progress {
                ThxFirmwareProgress::Command(command) => command.observed_firmware_version(),
                ThxFirmwareProgress::LocalEq(_) => None,
            })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThxOperationStatus {
    Running,
    Finished(ThxSubmissionProgress),
}

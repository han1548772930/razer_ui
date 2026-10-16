//! Current product-specific firmware button inventory and full profile reads.
//! Compile-time targeting data is separate from observed device assignments.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    mouse_button_mapping::{
        AssignmentMode, ButtonTarget, MouseButtonMapping, ObservedButtonAssignment,
    },
};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub struct MouseButtonInputDescriptor {
    pub input_id: &'static str,
    pub mode: AssignmentMode,
    pub firmware_id: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct MouseButtonInventoryDescriptor {
    pub product_id: u32,
    /// Actual current product BUTTON_LIST inputs with their selected mode.
    pub inputs: &'static [MouseButtonInputDescriptor],
    /// Source selector's unique, numerically sorted firmware IDs.
    pub firmware_ids: &'static [u8],
}

include!("mouse_button_inventory_data_generated.rs");

pub fn capability(product_id: u32) -> Option<&'static MouseButtonInventoryDescriptor> {
    AUDITED_BUTTON_INVENTORIES
        .iter()
        .find(|value| value.product_id == product_id)
}

/// Resolve a real product input name without inferring support from a shared
/// generic mapping table. Mode is selected separately, using the same inventory.
pub fn target(
    product_id: u32,
    profile_id: u8,
    input_id: &str,
    mode: AssignmentMode,
) -> Result<ButtonTarget> {
    let descriptor = capability(product_id).context("No current mouse button inventory")?;
    let input = descriptor
        .inputs
        .iter()
        .find(|input| input.input_id == input_id && input.mode == mode)
        .context("Input is absent from the current product button list")?;
    let target = ButtonTarget {
        profile_id,
        button_id: input
            .firmware_id
            .context("Current input has no source-derived firmware ID")?,
        mode,
    };
    target.validate()?;
    Ok(target)
}

/// Match the source's NormalMode pass followed by its HypershiftMode pass.
pub fn profile_targets(product_id: u32, profile_id: u8) -> Result<Vec<ButtonTarget>> {
    ensure!(profile_id > 0, "Mouse profile ID cannot be zero");
    let descriptor = capability(product_id).context("No current mouse button inventory")?;
    Ok([AssignmentMode::Normal, AssignmentMode::Hypershift]
        .into_iter()
        .flat_map(|mode| {
            descriptor
                .firmware_ids
                .iter()
                .map(move |&button_id| ButtonTarget {
                    profile_id,
                    button_id,
                    mode,
                })
        })
        .collect())
}

#[derive(Clone, Debug, Serialize)]
pub struct ButtonInventoryEntry {
    pub target: ButtonTarget,
    pub input_ids: Vec<&'static str>,
    /// The GET reached the transport; preflight failure does not set this flag.
    pub attempted: bool,
    pub observed: Option<ObservedButtonAssignment>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ButtonInventoryReport {
    pub product_id: u32,
    pub profile_id: u8,
    pub entries: Vec<ButtonInventoryEntry>,
    /// Both source passes reached their end. Individual failures remain visible.
    pub source_scan_completed: bool,
}

impl ButtonInventoryReport {
    pub fn complete(&self) -> bool {
        self.source_scan_completed
            && self
                .entries
                .iter()
                .all(|entry| entry.observed.is_some() && entry.error.is_none())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ButtonInventoryFailurePhase {
    Prepare,
    Read,
    Cancelled,
}

#[derive(Debug)]
pub struct ButtonInventoryFailure {
    pub phase: ButtonInventoryFailurePhase,
    pub progress: ButtonInventoryReport,
    pub failed_target: Option<ButtonTarget>,
    error: anyhow::Error,
}

impl std::fmt::Display for ButtonInventoryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse button inventory {:?} failed: {:#}",
            self.phase, self.error
        )
    }
}

impl std::error::Error for ButtonInventoryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

pub struct MouseButtonInventory<'a> {
    mapping: MouseButtonMapping<'a>,
    descriptor: &'static MouseButtonInventoryDescriptor,
}

impl<'a> MouseButtonInventory<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        transport: &'a DeviceReadCapability,
    ) -> Result<Self> {
        let descriptor =
            capability(transport.product_id).context("No current mouse button inventory")?;
        Ok(Self {
            mapping: MouseButtonMapping::new(device, transport)?,
            descriptor,
        })
    }

    /// Read actual firmware state for every source-derived target. Ordinary GET
    /// errors retain observations and continue; cancellation stops immediately.
    pub fn read_profile(
        &self,
        profile_id: u8,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ButtonInventoryReport, ButtonInventoryFailure> {
        let mut report = ButtonInventoryReport {
            product_id: self.descriptor.product_id,
            profile_id,
            entries: Vec::new(),
            source_scan_completed: false,
        };
        let targets = profile_targets(self.descriptor.product_id, profile_id).map_err(|error| {
            ButtonInventoryFailure {
                phase: ButtonInventoryFailurePhase::Prepare,
                progress: report.clone(),
                failed_target: None,
                error,
            }
        })?;
        report.entries = targets
            .into_iter()
            .map(|target| ButtonInventoryEntry {
                target,
                input_ids: self
                    .descriptor
                    .inputs
                    .iter()
                    .filter(|input| {
                        input.firmware_id == Some(target.button_id) && input.mode == target.mode
                    })
                    .map(|input| input.input_id)
                    .collect(),
                attempted: false,
                observed: None,
                error: None,
            })
            .collect();
        let mut last_failure = None;
        for index in 0..report.entries.len() {
            let target = report.entries[index].target;
            let attempted = std::cell::Cell::new(false);
            let cancelled = std::cell::Cell::new(false);
            let guarded = || validate().inspect_err(|_| cancelled.set(true));
            let observed = self.mapping.read_observing_send(
                target,
                &mut next_transaction,
                &guarded,
                &mut || attempted.set(true),
            );
            report.entries[index].attempted = attempted.get();
            match observed {
                Ok(observed) => report.entries[index].observed = Some(observed),
                Err(error) => {
                    report.entries[index].error = Some(format!("{error:#}"));
                    if cancelled.get() {
                        return Err(ButtonInventoryFailure {
                            phase: ButtonInventoryFailurePhase::Cancelled,
                            progress: report,
                            failed_target: Some(target),
                            error,
                        });
                    }
                    last_failure = Some((target, error));
                }
            }
        }
        report.source_scan_completed = true;
        if let Some((target, error)) = last_failure {
            return Err(ButtonInventoryFailure {
                phase: ButtonInventoryFailurePhase::Read,
                progress: report,
                failed_target: Some(target),
                error,
            });
        }
        Ok(report)
    }
}

#[cfg(test)]
#[path = "mouse_button_inventory_tests.rs"]
mod tests;

//! Current complete HID OBM reads retain the full physical request identity,
//! layout and local macro identity filter across the worker boundary.
pub use crate::keyboard_profile_inventory::Target;
use anyhow::ensure;
pub use razer_device::keyboard_onboard_profiles::{CompleteReading, ReadOptions};
use serde::{Deserialize, Serialize};

pub fn validate_target(target: &Target) -> anyhow::Result<()> {
    target.validate()?;
    ensure!(
        target.product_id() == razer_device::keyboard_onboard_profiles::PRODUCT_ID,
        "Keyboard full OBM reader is not captured for this product"
    );
    let (physical, peer) = match target {
        Target::Collection {
            node,
            peer_product_id,
            ..
        } => (u32::from(node.product_id), *peer_product_id),
        Target::WindowsContainer { target } => (target.physical_product_id, target.peer_product_id),
    };
    ensure!(
        physical == target.product_id() && peer.is_none(),
        "Current keyboard full OBM reader requires its mounted direct HID factory"
    );
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub target: Target,
    pub options: ReadOptions,
    pub reading: CompleteReading,
}

impl Observation {
    pub fn validate_for(&self, target: &Target, options: &ReadOptions) -> anyhow::Result<()> {
        validate_target(target)?;
        ensure!(
            &self.target == target && &self.options == options,
            "Keyboard OBM observation belongs to another physical request or layout"
        );
        self.reading.validate_for(options)
    }
}

#[cfg(test)]
#[path = "keyboard_onboard_profiles_tests.rs"]
mod tests;

//! Typed identity envelope for the current HID OBM inventory. Active hardware
//! slots and local profile selection are separate observations.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::HidNode, device_reads::DeviceReadTarget, keyboard_profile_inventory::ProfileInventory,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum Target {
    Collection {
        node: HidNode,
        product_id: u32,
        peer_product_id: Option<u32>,
    },
    WindowsContainer {
        target: DeviceReadTarget,
    },
}

impl Target {
    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection { product_id, .. } => *product_id,
            Self::WindowsContainer { target } => target.product_id,
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            razer_device::keyboard_profile_inventory::supports(self.product_id()),
            "Keyboard does not mount the current HID OBM inventory"
        );
        let (physical_product_id, peer_product_id) = match self {
            Self::Collection {
                node,
                peer_product_id,
                ..
            } => (u32::from(node.product_id), *peer_product_id),
            Self::WindowsContainer { target } => {
                (target.physical_product_id, target.peer_product_id)
            }
        };
        let capability = match razer_device::keyboard_metadata::hid_capability(
            self.product_id(),
            physical_product_id,
        ) {
            Some(capability) => std::borrow::Cow::Borrowed(capability),
            None if peer_product_id.is_some() => {
                std::borrow::Cow::Owned(razer_device::keyboard_metadata::relay_capability(
                    self.product_id(),
                    physical_product_id,
                )?)
            }
            None => anyhow::bail!("Keyboard inventory has no current HID factory"),
        };
        if let Some(peer) = peer_product_id {
            let radio = razer_device::keyboard_metadata::radio_capability(self.product_id())
                .context("Keyboard inventory peer has no current radio factory")?;
            ensure!(
                peer == self.product_id() || radio.direct_pids.contains(&peer),
                "Keyboard inventory peer differs from its radio factory"
            );
        }
        match self {
            Self::Collection { node, .. } => ensure!(
                !node.path.is_empty()
                    && node.vendor_id == capability.vendor_id
                    && node.interface_number
                        == i32::from(capability.claim_interface_for(physical_product_id)?),
                "Keyboard inventory collection differs from its HID factory"
            ),
            Self::WindowsContainer { target } => {
                ensure!(
                    !target.path.is_empty() && !target.device_container_id.is_empty(),
                    "Keyboard inventory container identity is incomplete"
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub target: Target,
    pub inventory: ProfileInventory,
}

impl Observation {
    pub fn validate_for(&self, target: &Target) -> anyhow::Result<()> {
        target.validate()?;
        ensure!(
            &self.target == target,
            "Keyboard profile inventory belongs to another physical owner"
        );
        self.inventory.validate(target.product_id())
    }
}

#[cfg(test)]
#[path = "keyboard_profile_inventory_tests.rs"]
mod tests;

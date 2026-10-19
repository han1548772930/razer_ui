//! Current OBM identity inventory over the audited keyboard metadata route.
use super::{DirectRoute, keyboard_metadata};
use crate::discovery::{ObservedDevice, ObservedTransport};
use anyhow::ensure;
use razer_device::keyboard_profile_inventory::ProfileInventory;
pub use razer_ipc::keyboard_profile_inventory::Target;
use razer_ipc::{ServiceClient, ServiceRequest, keyboard_profile_inventory::Observation};

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Target> {
    ensure!(
        razer_device::keyboard_profile_inventory::supports(observed.product_id())
            && matches!(
                observed.transport(),
                Some(ObservedTransport::Wired | ObservedTransport::Dongle)
            ),
        "Keyboard OBM inventory is not mounted for this product/transport"
    );
    let target = match keyboard_metadata::resolve(client, observed)? {
        keyboard_metadata::Route::Hid(DirectRoute::Collection { node, product_id }) => {
            Target::Collection {
                node,
                product_id,
                peer_product_id: None,
            }
        }
        keyboard_metadata::Route::RelayCollection {
            node,
            product_id,
            peer_product_id,
        } => Target::Collection {
            node,
            product_id,
            peer_product_id: Some(peer_product_id),
        },
        #[cfg(windows)]
        keyboard_metadata::Route::Hid(DirectRoute::Container(target)) => {
            Target::WindowsContainer { target }
        }
        #[cfg(windows)]
        keyboard_metadata::Route::Ble(_) => {
            anyhow::bail!("Current keyboard runtime excludes BLE OBM initialization")
        }
    };
    target.validate()?;
    Ok(target)
}

pub fn read(client: &mut ServiceClient, target: &Target) -> anyhow::Result<ProfileInventory> {
    read_with(target, |request| client.request(request))
}

fn read_with(
    target: &Target,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
) -> anyhow::Result<ProfileInventory> {
    target.validate()?;
    let reply = request(ServiceRequest::KeyboardProfileInventoryRead {
        target: target.clone(),
    })?;
    let observation: Observation = serde_json::from_value(reply)?;
    observation.validate_for(target)?;
    Ok(observation.inventory)
}

#[cfg(test)]
#[path = "keyboard_profile_inventory_tests.rs"]
mod tests;

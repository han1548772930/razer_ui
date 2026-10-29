//! OS observations become a complete collection identity at this boundary.
use super::collection::{CollectionIdentity, PlatformIdentity};
use razer_device::backend::HidNode;

pub(super) fn resolve(node: &HidNode) -> anyhow::Result<CollectionIdentity> {
    #[cfg(windows)]
    {
        parse(
            node,
            super::super::native::broker_collection_metadata(node)?,
        )
    }
    #[cfg(not(windows))]
    {
        Ok(CollectionIdentity {
            node: node.clone(),
            platform: PlatformIdentity::Portable,
        })
    }
}

#[cfg(any(windows, test))]
use anyhow::{Context as _, ensure};
#[cfg(any(windows, test))]
use serde::Deserialize;

#[cfg(any(windows, test))]
#[derive(Deserialize)]
struct Inventory {
    interfaces: Vec<Interface>,
    failures: Vec<Failure>,
    complete: bool,
}

#[cfg(any(windows, test))]
#[derive(Deserialize)]
struct Failure {
    operation: String,
    error: String,
}

// deserialize_with requires the nullable field to be present, distinguishing
// an explicit native failure from an incompatible or truncated wire schema.
#[cfg(any(windows, test))]
fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[cfg(any(windows, test))]
#[derive(Deserialize)]
struct Interface {
    path: String,
    vendor_id: u16,
    product_id: u16,
    version_number: u16,
    #[serde(deserialize_with = "nullable")]
    device_container_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    container_id_error: Option<String>,
    #[serde(deserialize_with = "nullable")]
    device_instance_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    device_instance_id_error: Option<String>,
    #[serde(deserialize_with = "nullable")]
    parent_instance_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    parent_instance_id_error: Option<String>,
    #[serde(deserialize_with = "nullable")]
    claim_interface: Option<i32>,
    #[serde(deserialize_with = "nullable")]
    claim_interface_error: Option<String>,
    #[serde(deserialize_with = "nullable")]
    usage_page: Option<u16>,
    #[serde(deserialize_with = "nullable")]
    usage: Option<u16>,
}

#[cfg(any(windows, test))]
fn parse(node: &HidNode, raw: serde_json::Value) -> anyhow::Result<CollectionIdentity> {
    let inventory: Inventory =
        serde_json::from_value(raw).context("HID identity schema is invalid")?;
    ensure!(
        inventory.complete,
        "HID identity enumeration did not complete"
    );
    ensure!(
        inventory.failures.is_empty(),
        "HID identity enumeration failed: {}",
        inventory
            .failures
            .iter()
            .map(|error| format!("{}: {}", error.operation, error.error))
            .collect::<Vec<_>>()
            .join("; ")
    );
    ensure!(
        inventory.interfaces.len() == 1,
        "HID identity must resolve exactly one collection"
    );
    let observed = inventory
        .interfaces
        .into_iter()
        .next()
        .expect("one collection");
    let path = std::str::from_utf8(&node.path).context("Windows HID path is not UTF-8")?;
    ensure!(
        observed.path.is_ascii() && observed.path.eq_ignore_ascii_case(path),
        "HID identity path differs"
    );
    ensure!(
        (
            observed.vendor_id,
            observed.product_id,
            observed.version_number
        ) == (node.vendor_id, node.product_id, node.release_number),
        "HID identity product or version differs"
    );
    ensure!(
        observed.usage_page == Some(node.usage_page) && observed.usage == Some(node.usage),
        "HID identity usage was not observed or differs"
    );
    ensure!(
        observed.container_id_error.is_none()
            && observed.device_instance_id_error.is_none()
            && observed.parent_instance_id_error.is_none(),
        "HID physical identity could not be observed: container={:?}, instance={:?}, parent={:?}",
        observed.container_id_error,
        observed.device_instance_id_error,
        observed.parent_instance_id_error
    );
    if node.interface_number >= 0 {
        ensure!(
            observed.claim_interface_error.is_none()
                && observed.claim_interface == Some(node.interface_number),
            "HID interface number differs or was not observed"
        );
    } else {
        ensure!(
            observed.claim_interface.is_none(),
            "HID interface observation contradicts the selected node"
        );
    }
    let container_id = observed
        .device_container_id
        .context("HID ContainerId is unobserved")?;
    let hid_instance_id = observed
        .device_instance_id
        .context("HID instance is unobserved")?;
    let parent_instance_id = observed
        .parent_instance_id
        .context("HID parent instance is unobserved")?;
    for identity in [&container_id, &hid_instance_id, &parent_instance_id] {
        ensure!(
            !identity.is_empty() && !identity.contains('\0'),
            "HID physical identity is empty or invalid"
        );
    }
    Ok(CollectionIdentity {
        node: node.clone(),
        platform: PlatformIdentity::Windows {
            container_id,
            hid_instance_id,
            parent_instance_id,
        },
    })
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;

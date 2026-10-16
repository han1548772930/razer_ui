//! Mouse runtime artwork chooses its real wired/dongle/receiver peer route.
use super::*;
use razer_device::mouse_runtime_artwork::{self as artwork, Target};

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Target> {
    let source =
        artwork::source(observed.product_id()).context("Mouse artwork current source is absent")?;
    let physical = observed.physical_product_id();
    let relay = observed.peer_product_id().is_some();
    if matches!(
        observed.transport(),
        Some(crate::discovery::ObservedTransport::Ble)
    ) {
        ensure!(
            !relay && source.is_ble(physical),
            "Mouse BLE owner differs from the current source branch"
        );
        #[cfg(windows)]
        {
            let hid = client.request(ServiceRequest::HidDevices)?;
            return Ok(Target::WindowsBle(artwork::ble_target_from_hid(
                observed.product_id(),
                physical,
                observed.container(),
                &hid,
            )?));
        }
        #[cfg(not(windows))]
        anyhow::bail!("Mouse BLE Protocol30 requires a platform GATT adapter");
    }
    ensure!(
        !relay || source.supports_relay(),
        "Mouse artwork source has no current DualLink relay branch"
    );
    ensure!(
        matches!(
            observed.transport(),
            Some(
                crate::discovery::ObservedTransport::Wired
                    | crate::discovery::ObservedTransport::Dongle
            )
        ),
        "Mouse artwork requires its actual source HID route"
    );
    ensure!(
        if let Some(peer) = observed.peer_product_id() {
            source.accepts(peer) && !source.is_ble(peer)
        } else {
            source.accepts(physical) && !source.is_ble(physical)
        },
        "Mouse artwork observed physical/peer route differs"
    );
    let mut cap = source.transport(physical, relay)?;
    if relay {
        let receiver = razer_device::receiver_capabilities::capability(u16::try_from(physical)?)
            .context("Mouse artwork receiver protocol is absent")?;
        cap.claim_interfaces_by_pid
            .insert(physical.to_string(), receiver.claim_interface);
    }
    if let Some(node) = observed.hid_node() {
        ensure!(
            !relay
                && node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == physical
                && node.interface_number == i32::from(cap.claim_interface_for(physical)?),
            "Mouse artwork observed collection differs"
        );
        let current = client.request(ServiceRequest::HidNodes)?;
        let nodes: Vec<HidNode> = serde_json::from_value(current["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Mouse artwork observed collection changed"
        );
        source_reports_match(client, node, 91)?;
        return Ok(Target::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        });
    }
    #[cfg(windows)]
    {
        let mut target = windows::resolve(client, observed, &cap)?;
        target.peer_product_id = observed.peer_product_id();
        return Ok(Target::WindowsContainer(target));
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Mouse artwork physical ContainerId selection requires a platform adapter")
    }
}

use super::*;
use razer_device::earbuds_auto_pause::{self, Operation, Report};

pub fn event_target(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<razer_device::earbuds_events::Target> {
    let cap = earbuds_auto_pause::transport(observed.product_id())?;
    ensure!(
        observed.physical_product_id() == observed.product_id()
            && observed.peer_product_id().is_none(),
        "Earbud event owner differs from its direct dongle"
    );
    if let Some(node) = observed.hid_node() {
        return Ok(razer_device::earbuds_events::Target::Collection(
            node.clone(),
        ));
    }
    #[cfg(windows)]
    {
        Ok(razer_device::earbuds_events::Target::WindowsContainer(
            windows::resolve(client, observed, &cap)?,
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = (client, cap);
        anyhow::bail!("Earbud events require an observed collection");
    }
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    operation: Operation,
) -> anyhow::Result<Report> {
    let cap = earbuds_auto_pause::transport(observed.product_id())?;
    ensure!(
        observed.physical_product_id() == observed.product_id()
            && observed.peer_product_id().is_none()
            && observed.transport() != Some(crate::discovery::ObservedTransport::Ble),
        "Earbud auto-pause requires its current USB dongle"
    );
    let request = if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == cap.product_id
                && node.interface_number == i32::from(cap.claim_interface),
            "Earbud collection differs"
        );
        source_reports_match(client, node, cap.report_bytes)?;
        ServiceRequest::HidNodeEarbudsAutoPause {
            node: node.clone(),
            product_id: cap.product_id,
            auto_pause_operation: operation,
        }
    } else {
        #[cfg(windows)]
        {
            ServiceRequest::DeviceEarbudsAutoPause {
                target: windows::resolve(client, observed, &cap)?,
                auto_pause_operation: operation,
            }
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Earbud observation has no portable HID collection");
        }
    };
    let reply: Report = serde_json::from_value(client.request(request)?)?;
    ensure!(
        reply.product_id == observed.product_id() && reply.operation == operation,
        "Earbud result belongs to another task"
    );
    Ok(reply)
}

//! The single status LED belongs to the actual receiver, never a paired peer.
use super::*;
use razer_device::mouse_receiver_legacy::{self as indicator, Change, Report, Target};
pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Target> {
    let cap = indicator::capability(observed.product_id())
        .context("Single receiver LED source is absent")?;
    ensure!(
        indicator::accepts(
            observed.product_id(),
            observed.physical_product_id(),
            observed.peer_product_id()
        ) && matches!(
            observed.transport(),
            Some(crate::discovery::ObservedTransport::Dongle)
        ),
        "Single LED has no physical receiver observation"
    );
    resolve_capability(client, observed, cap)
}
pub fn resolve_bindings(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<Target> {
    let cap = indicator::binding_capability(observed.product_id(), observed.physical_product_id())
        .context("Current binding getter has no verified runtime adapter")?;
    ensure!(
        observed.peer_product_id().is_none()
            && matches!(
                observed.transport(),
                Some(
                    crate::discovery::ObservedTransport::Wired
                        | crate::discovery::ObservedTransport::Dongle
                )
            ),
        "Binding query has no actual runtime device observation"
    );
    resolve_capability(client, observed, cap)
}
fn resolve_capability(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    cap: &razer_device::device_reads::DeviceReadCapability,
) -> anyhow::Result<Target> {
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == observed.physical_product_id()
                && node.interface_number == i32::from(cap.claim_interface),
            "Single LED receiver collection differs"
        );
        let nodes: Vec<HidNode> =
            serde_json::from_value(client.request(ServiceRequest::HidNodes)?["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|value| *value == node).count() == 1,
            "Single LED receiver collection changed or is ambiguous"
        );
        super::source_reports_match(client, node, cap.report_bytes)?;
        return Ok(Target::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        });
    }
    #[cfg(windows)]
    {
        Ok(Target::WindowsContainer(windows::resolve(
            client, observed, cap,
        )?))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Single receiver LED needs an observed HID collection on this platform")
    }
}
pub fn execute(
    client: &mut ServiceClient,
    target: &Target,
    change: Change,
) -> anyhow::Result<Report> {
    let reply = client.request(ServiceRequest::MouseReceiverLegacy {
        target: target.clone(),
        change,
    })?;
    ensure!(
        reply["target"] == serde_json::to_value(target)?,
        "Single LED reply belongs to another receiver"
    );
    serde_json::from_value(reply["report"].clone()).map_err(Into::into)
}

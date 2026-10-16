//! Actual physical receiver selection for the keyboard's single status LED.
use super::*;
use razer_device::keyboard_indicator::{self as indicator, Change, Report, Target};
pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Target> {
    ensure!(
        observed.peer_product_id().is_none()
            && matches!(
                observed.transport(),
                Some(crate::discovery::ObservedTransport::Dongle)
            ),
        "Keyboard indicator has no actual physical receiver observation"
    );
    let cap = indicator::capability(observed.product_id(), observed.physical_product_id())?;
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == indicator::RECEIVER_ID
                && node.interface_number == 2,
            "Keyboard indicator collection differs from source"
        );
        let nodes: Vec<HidNode> =
            serde_json::from_value(client.request(ServiceRequest::HidNodes)?["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|candidate| *candidate == node).count() == 1,
            "Keyboard indicator receiver collection changed or is ambiguous"
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
            client, observed, &cap,
        )?))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Keyboard indicator requires a current HID collection on this platform")
    }
}
pub fn execute(
    client: &mut ServiceClient,
    target: Target,
    change: Change,
) -> anyhow::Result<Report> {
    let reply = client.request(ServiceRequest::KeyboardIndicator {
        target: target.clone(),
        change,
    })?;
    ensure!(
        reply["target"] == serde_json::to_value(target)?,
        "Keyboard indicator reply owner differs"
    );
    serde_json::from_value(reply["report"].clone()).map_err(Into::into)
}

//! Current mounted magnetic calibration is a wired interface-3 operation.
use super::*;
use razer_device::keyboard_legacy_calibration::{self as calibration, Operation, Report, Target};
pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Target> {
    ensure!(
        observed.product_id() == observed.physical_product_id()
            && observed.peer_product_id().is_none()
            && matches!(
                observed.transport(),
                Some(crate::discovery::ObservedTransport::Wired)
            ),
        "Magnetic calibration requires its current physical wired keyboard"
    );
    let cap = calibration::capability(observed.product_id())?;
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == observed.product_id()
                && node.interface_number == 3,
            "Magnetic calibration HID identity differs"
        );
        let nodes: Vec<HidNode> =
            serde_json::from_value(client.request(ServiceRequest::HidNodes)?["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Calibration collection changed or is ambiguous"
        );
        source_reports_match(client, node, cap.report_bytes)?;
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
        anyhow::bail!("Magnetic calibration requires a current HID collection on this platform")
    }
}
pub fn execute(
    client: &mut ServiceClient,
    target: Target,
    operation: Operation,
) -> anyhow::Result<Report> {
    let reply = client.request(ServiceRequest::KeyboardLegacyCalibration {
        target: target.clone(),
        calibration_operation: operation,
    })?;
    ensure!(
        reply["target"] == serde_json::to_value(target)?,
        "Calibration reply owner differs"
    );
    serde_json::from_value(reply["report"].clone()).map_err(Into::into)
}

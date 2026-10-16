//! Current mouse basic power route retains the physical device and real peer.
use super::*;
use razer_device::mouse_current_power::{self as power, Change, Report};

#[derive(Clone)]
pub enum Route {
    Hid(DirectRoute),
    #[cfg(windows)]
    WindowsBle(razer_device::mouse_runtime_artwork::WindowsBleTarget),
}

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Route> {
    let source =
        power::capability(observed.product_id()).context("Current power source is absent")?;
    if matches!(
        observed.transport(),
        Some(crate::discovery::ObservedTransport::Ble)
    ) {
        ensure!(
            observed.peer_product_id().is_none() && source.is_ble(observed.physical_product_id()),
            "Current BLE power physical owner differs"
        );
        #[cfg(windows)]
        {
            let hid = client.request(ServiceRequest::HidDevices)?;
            return Ok(Route::WindowsBle(power::ble::target_from_hid(
                observed.product_id(),
                observed.physical_product_id(),
                observed.container(),
                &hid,
            )?));
        }
        #[cfg(not(windows))]
        anyhow::bail!("Mouse power BLE requires a platform GATT adapter");
    }
    ensure!(
        matches!(
            observed.transport(),
            Some(
                crate::discovery::ObservedTransport::Wired
                    | crate::discovery::ObservedTransport::Dongle
            )
        ) && source.accepts(observed.physical_product_id(), observed.peer_product_id()),
        "Current power physical route differs"
    );
    let cap = source.transport(
        observed.physical_product_id(),
        observed.peer_product_id().is_some(),
    )?;
    if let Some(node) = observed.hid_node() {
        ensure!(
            observed.peer_product_id().is_none()
                && node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == observed.physical_product_id()
                && node.interface_number
                    == i32::from(cap.claim_interface_for(observed.physical_product_id())?),
            "Power collection interface differs"
        );
        let nodes: Vec<HidNode> =
            serde_json::from_value(client.request(ServiceRequest::HidNodes)?["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Power collection changed or is ambiguous"
        );
        source_reports_match(client, node, cap.report_bytes)?;
        return Ok(Route::Hid(DirectRoute::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        }));
    }
    #[cfg(windows)]
    {
        let mut target = windows::resolve(client, observed, &cap)?;
        target.peer_product_id = observed.peer_product_id();
        Ok(Route::Hid(DirectRoute::Container(target)))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Power ContainerId route needs its platform HID adapter")
    }
}
impl Route {
    pub fn mouse_current_power(
        &self,
        client: &mut ServiceClient,
        change: Change,
    ) -> anyhow::Result<Report> {
        match self {
            Self::Hid(route) => route.mouse_current_power(client, change),
            #[cfg(windows)]
            Self::WindowsBle(target) => {
                power::validate_change(target.product_id, &change)?;
                let reply = client.request(ServiceRequest::BleMouseCurrentPower {
                    target: target.clone(),
                    change: change.clone(),
                })?;
                ensure!(
                    reply["target"] == serde_json::to_value(target)?
                        && reply["setting_scope"] == "basic_device",
                    "BLE power service identity/scope differs"
                );
                let report: Report = serde_json::from_value(reply["report"].clone())?;
                ensure!(
                    report.requested == change && !report.acknowledgements.is_empty(),
                    "BLE power service did not confirm its exact source task"
                );
                Ok(report)
            }
        }
    }
}
impl DirectRoute {
    pub fn mouse_current_power(
        &self,
        client: &mut ServiceClient,
        change: Change,
    ) -> anyhow::Result<Report> {
        let product_id = match self {
            Self::Collection { product_id, .. } => *product_id,
            #[cfg(windows)]
            Self::Container(target) => target.product_id,
        };
        power::validate_change(product_id, &change)?;
        let request = match self {
            Self::Collection { node, product_id } => ServiceRequest::HidNodeMouseCurrentPower {
                node: node.clone(),
                product_id: *product_id,
                change: change.clone(),
            },
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceMouseCurrentPower {
                target: target.clone(),
                change: change.clone(),
            },
        };
        let reply = client.request(request)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["setting_scope"] == "basic_device",
            "Power service setting scope differs"
        );
        let report: Report = serde_json::from_value(reply["report"].clone())?;
        ensure!(
            report.requested == change && !report.acknowledgements.is_empty(),
            "Power response does not confirm its exact request"
        );
        Ok(report)
    }
}

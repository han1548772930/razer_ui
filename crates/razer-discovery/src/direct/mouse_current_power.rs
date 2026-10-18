//! Current mouse basic power route retains the physical device and real peer.
use super::*;
use razer_device::mouse_current_power::{self as power, Change, Report};

#[derive(Clone)]
pub enum Route {
    Hid(DirectRoute),
    Receiver {
        route: DirectRoute,
        peer_product_id: u32,
    },
    #[cfg(windows)]
    WindowsBle(razer_device::mouse_runtime_artwork::WindowsBleTarget),
}

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Route> {
    resolve_with(observed, &mut |request| client.request(request))
}

fn resolve_with(
    observed: &ObservedDevice,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Route> {
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
            let hid = request(ServiceRequest::HidDevices)?;
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
    if observed.peer_product_id().is_some() {
        ensure!(
            observed.transport() == Some(crate::discovery::ObservedTransport::Dongle)
                && observed.connection()
                    == razer_model::model::DeviceConnectionObservation::ReceiverPeer(1),
            "Current power requires an online receiver observation"
        );
    }
    let cap = source.transport(
        observed.physical_product_id(),
        observed.peer_product_id().is_some(),
    )?;
    if let Some(node) = observed.hid_node() {
        if let Some(peer_product_id) = observed.peer_product_id() {
            let session =
                source.receiver_session(observed.physical_product_id(), peer_product_id)?;
            return Ok(Route::Receiver {
                route: super::receiver_settings::resolve(observed, &session, request)?,
                peer_product_id,
            });
        }
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == observed.physical_product_id()
                && node.interface_number
                    == i32::from(cap.claim_interface_for(observed.physical_product_id())?),
            "Power collection interface differs"
        );
        let nodes: Vec<HidNode> =
            serde_json::from_value(request(ServiceRequest::HidNodes)?["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Power collection changed or is ambiguous"
        );
        source_reports_match_with(node, cap.report_bytes, request)?;
        return Ok(Route::Hid(DirectRoute::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        }));
    }
    #[cfg(windows)]
    {
        let mut target = windows::resolve_with(observed, &cap, request)?;
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
        self.mouse_current_power_with(change, &mut |request| client.request(request))
    }

    fn mouse_current_power_with(
        &self,
        change: Change,
        request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<Report> {
        match self {
            Self::Hid(route) => route.mouse_current_power_with(None, change, request),
            Self::Receiver {
                route,
                peer_product_id,
            } => route.mouse_current_power_with(Some(*peer_product_id), change, request),
            #[cfg(windows)]
            Self::WindowsBle(target) => {
                power::validate_change(target.product_id, &change)?;
                let reply = request(ServiceRequest::BleMouseCurrentPower {
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
        self.mouse_current_power_with(None, change, &mut |request| client.request(request))
    }

    fn mouse_current_power_with(
        &self,
        peer_product_id: Option<u32>,
        change: Change,
        request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<Report> {
        let (product_id, physical_product_id, peer_product_id) = match self {
            Self::Collection { product_id, node } => {
                (*product_id, u32::from(node.product_id), peer_product_id)
            }
            #[cfg(windows)]
            Self::Container(target) => (
                target.product_id,
                target.physical_product_id,
                target.peer_product_id,
            ),
        };
        power::validate_change(product_id, &change)?;
        let source = power::capability(product_id).context("Current power source is absent")?;
        ensure!(
            source.accepts(physical_product_id, peer_product_id),
            "Current power request physical/peer identity differs"
        );
        let cap = source.transport(physical_product_id, peer_product_id.is_some())?;
        let session = if matches!(self, Self::Collection { .. }) {
            peer_product_id
                .map(|peer| source.receiver_session(physical_product_id, peer))
                .transpose()?
        } else {
            None
        };
        let query = match self {
            Self::Collection { node, product_id } => ServiceRequest::HidNodeMouseCurrentPower {
                node: node.clone(),
                product_id: *product_id,
                peer_product_id,
                change: change.clone(),
            },
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceMouseCurrentPower {
                target: target.clone(),
                change: change.clone(),
            },
        };
        let reply = request(query)?;
        self.check_reply(&reply)?;
        if let Some(session) = &session {
            ensure!(
                reply["peer_product_id"] == serde_json::to_value(peer_product_id)?,
                "Power response belongs to another receiver peer"
            );
            super::receiver_settings::check_peers(&reply, session)?;
        }
        ensure!(
            reply["setting_scope"] == "basic_device" && reply["source_class"] == cap.source_class,
            "Power service setting scope or source differs"
        );
        let report: Report = serde_json::from_value(reply["report"].clone())?;
        ensure!(
            report.requested == change && !report.acknowledgements.is_empty(),
            "Power response does not confirm its exact request"
        );
        Ok(report)
    }
}

#[cfg(test)]
#[path = "mouse_current_power_tests.rs"]
mod tests;

//! The 1352 Noa route retains its actual physical collection/ContainerId.
use super::*;
use razer_device::leviathan::{self, Operation, Report};

pub fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<DirectRoute> {
    ensure!(
        observed.product_id() == 1352
            && observed.physical_product_id() == 1352
            && observed.peer_product_id().is_none(),
        "Noa source has no receiver route"
    );
    let cap = leviathan::transport();
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == 5426 && node.product_id == 1352 && node.interface_number == 4,
            "Noa collection differs"
        );
        let nodes: Vec<HidNode> =
            serde_json::from_value(client.request(ServiceRequest::HidNodes)?["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Noa collection changed or is ambiguous"
        );
        source_reports_match(client, node, cap.report_bytes)?;
        return Ok(DirectRoute::Collection {
            node: node.clone(),
            product_id: 1352,
        });
    }
    #[cfg(windows)]
    {
        Ok(DirectRoute::Container(windows::resolve(
            client, observed, cap,
        )?))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Noa container route requires its platform HID adapter")
    }
}
impl DirectRoute {
    pub fn leviathan_event_target(&self)->anyhow::Result<leviathan::EventTarget> {
        Ok(match self {
            Self::Collection{node,product_id}=>{ensure!(*product_id==1352,"Noa event product differs");leviathan::EventTarget::Collection(node.clone())},
            #[cfg(windows)]Self::Container(target)=>{ensure!(target.product_id==1352,"Noa event product differs");leviathan::EventTarget::WindowsContainer(target.clone())},
        })
    }
    pub fn leviathan(
        &self,
        client: &mut ServiceClient,
        operation: Operation,
    ) -> anyhow::Result<Report> {
        operation.validate()?;
        let request = match self {
            Self::Collection { node, product_id } => {
                ensure!(*product_id == 1352, "Noa product differs");
                ServiceRequest::HidNodeLeviathan {
                    node: node.clone(),
                    command: operation,
                }
            }
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceLeviathan {
                target: target.clone(),
                command: operation,
            },
        };
        let reply = client.request(request)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["setting_scope"] == "sound",
            "Noa service scope differs"
        );
        let report: Report = serde_json::from_value(reply["report"].clone())?;
        ensure!(
            report.requested == operation && !report.acknowledgements.is_empty(),
            "Noa service task reply differs"
        );
        Ok(report)
    }
}

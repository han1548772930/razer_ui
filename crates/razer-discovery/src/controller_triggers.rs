//! Current controller feature collection selected by observed container/path.
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::{backend::HidNode, controller_triggers::capability};
use razer_ipc::{ServiceClient, ServiceRequest};

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<HidNode> {
    let source =
        capability(observed.product_id()).context("No current controller trigger route")?;
    ensure!(
        source.accepts(observed.physical_product_id()) && observed.peer_product_id().is_none(),
        "Controller trigger physical identity differs"
    );
    let reply = client.request(ServiceRequest::HidNodes)?;
    let nodes: Vec<HidNode> = serde_json::from_value(reply["nodes"].clone())?;
    let matching = if let Some(retained) = observed.hid_node() {
        nodes
            .iter()
            .filter(|node| *node == retained)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        #[cfg(windows)]
        {
            let reply = client.request(ServiceRequest::HidDevices)?;
            let interfaces = reply["interfaces"]
                .as_array()
                .context("Missing Windows HID interfaces")?;
            nodes
                .iter()
                .filter(|node| {
                    interfaces.iter().any(|item| {
                        item["vendor_id"] == 5426
                            && item["product_id"] == observed.physical_product_id()
                            && item["claim_interface"] == 1
                            && item["feature_report_bytes"] == 91
                            && item["device_container_id"]
                                .as_str()
                                .is_some_and(|id| id.eq_ignore_ascii_case(observed.container()))
                            && item["path"].as_str().is_some_and(|path| {
                                std::str::from_utf8(&node.path)
                                    .is_ok_and(|p| p.eq_ignore_ascii_case(path))
                            })
                    })
                })
                .cloned()
                .collect::<Vec<_>>()
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("The controller has no descriptor-observed HID collection identity");
        }
    };
    let [node] = matching.as_slice() else {
        anyhow::bail!("Controller has no unique current feature collection");
    };
    ensure!(
        node.vendor_id == 5426
            && u32::from(node.product_id) == observed.physical_product_id()
            && matches!(node.interface_number, 1 | -1)
            && !node.path.is_empty(),
        "Controller trigger collection differs from current source"
    );
    crate::direct::reports_match(client, node, source.report_id(), 91)?;
    Ok(node.clone())
}

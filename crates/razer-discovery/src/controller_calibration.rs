//! Current SagePC Report10 route, independent of the ordinary Report0 queries.
use crate::discovery::{ObservedDevice, ObservedTransport};
use anyhow::{Context as _, ensure};
use razer_device::backend::HidNode;
use razer_ipc::{ServiceClient, ServiceRequest};

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<HidNode> {
    let physical: &[u32] = match observed.product_id() {
        2676 => &[2676, 2677],
        2684 => &[2684, 2685],
        _ => anyhow::bail!("Product has no recovered current V3 trigger calibration transport"),
    };
    ensure!(
        physical.contains(&observed.physical_product_id())
            && observed.peer_product_id().is_none()
            && matches!(observed.transport(), Some(ObservedTransport::Wired)),
        "Wireless calibration identity adapter is not implemented"
    );
    let reply = client.request(ServiceRequest::HidNodes)?;
    let nodes: Vec<HidNode> = serde_json::from_value(reply["nodes"].clone())?;
    let node = if let Some(retained) = observed.hid_node() {
        ensure!(
            nodes.iter().filter(|node| *node == retained).count() == 1,
            "Retained calibration collection has changed or is ambiguous"
        );
        retained.clone()
    } else {
        #[cfg(windows)]
        {
            resolve_container(client, observed, &nodes)?
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("No current platform collection identity for calibration")
        }
    };
    ensure!(
        node.vendor_id == 5426
            && u32::from(node.product_id) == observed.physical_product_id()
            && matches!(node.interface_number, 1 | -1)
            && !node.path.is_empty(),
        "Calibration collection does not match the current source interface"
    );
    crate::direct::reports_match(client, &node, 10, 91)?;
    Ok(node)
}

#[cfg(windows)]
fn resolve_container(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    nodes: &[HidNode],
) -> anyhow::Result<HidNode> {
    let reply = client.request(ServiceRequest::HidDevices)?;
    let interfaces = reply["interfaces"]
        .as_array()
        .context("Missing Windows HID interfaces")?;
    // Container association comes from the actual Windows enumeration. A VID/PID
    // match alone cannot select one of multiple physically identical controllers.
    let paths = interfaces
        .iter()
        .filter(|interface| {
            interface["vendor_id"].as_u64() == Some(5426)
                && interface["product_id"].as_u64()
                    == Some(u64::from(observed.physical_product_id()))
                && interface["claim_interface"].as_i64() == Some(1)
                && interface["feature_report_bytes"].as_u64() == Some(91)
                && interface["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(observed.container()))
        })
        .filter_map(|interface| interface["path"].as_str())
        .collect::<Vec<_>>();
    let matching = nodes
        .iter()
        .filter(|node| {
            node.vendor_id == 5426
                && u32::from(node.product_id) == observed.physical_product_id()
                && matches!(node.interface_number, 1 | -1)
                && std::str::from_utf8(&node.path).is_ok_and(|path| {
                    paths
                        .iter()
                        .any(|current| current.eq_ignore_ascii_case(path))
                })
        })
        .collect::<Vec<_>>();
    let [node] = matching.as_slice() else {
        anyhow::bail!("Container has no unique observed SagePC collection")
    };
    Ok((*node).clone())
}

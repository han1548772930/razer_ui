//! The descriptor-observed controller collection is resolved before each local
//! feature command. Logical product, physical product and container stay distinct.
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::backend::HidNode;
use razer_ipc::{ServiceClient, ServiceRequest};
use serde::Deserialize;

#[derive(Deserialize)]
struct NodesReply {
    nodes: Vec<HidNode>,
}

#[cfg(any(windows, test))]
#[derive(Deserialize)]
struct WindowsInterface {
    vendor_id: u16,
    product_id: u32,
    claim_interface: Option<i32>,
    feature_report_bytes: Option<u16>,
    device_container_id: Option<String>,
    path: String,
}

#[cfg(any(windows, test))]
impl WindowsInterface {
    fn matches(&self, node: &HidNode, physical_product_id: u32, container: &str, interface: i32) -> bool {
        self.vendor_id == 5426
            && self.product_id == physical_product_id
            && self.claim_interface == Some(interface)
            && self.feature_report_bytes == Some(91)
            && self.device_container_id.as_deref().is_some_and(|observed_container| {
                observed_container.eq_ignore_ascii_case(container)
            })
            && std::str::from_utf8(&node.path)
                .is_ok_and(|path| path.eq_ignore_ascii_case(&self.path))
    }
}

pub(super) fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    report_id: u8,
    claim_interface: i32,
) -> anyhow::Result<HidNode> {
    ensure!(
        observed.peer_product_id().is_none(),
        "Controller feature route is not a direct physical product"
    );
    let reply: NodesReply = serde_json::from_value(client.request(ServiceRequest::HidNodes)?)
        .context("Invalid controller HID collection observation")?;
    let matching = if let Some(retained) = observed.hid_node() {
        reply.nodes.into_iter().filter(|node| node == retained).collect::<Vec<_>>()
    } else {
        #[cfg(windows)]
        {
            #[derive(Deserialize)]
            struct InterfacesReply {
                interfaces: Vec<WindowsInterface>,
            }
            let reply_interfaces: InterfacesReply =
                serde_json::from_value(client.request(ServiceRequest::HidDevices)?)
                    .context("Invalid Windows controller interface observation")?;
            reply.nodes.into_iter().filter(|node| {
                reply_interfaces.interfaces.iter().any(|candidate| {
                    candidate.matches(node, observed.physical_product_id(), observed.container(), claim_interface)
                })
            }).collect::<Vec<_>>()
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
            && (node.interface_number == claim_interface
                || cfg!(windows) && node.interface_number == -1)
            && !node.path.is_empty(),
        "Controller feature collection differs from current source"
    );
    crate::direct::reports_match(client, node, report_id, 91)?;
    Ok(node.clone())
}

#[cfg(test)]
mod tests {
    use super::{HidNode, NodesReply, WindowsInterface};

    fn node() -> HidNode {
        HidNode {
            path: br"\\?\hid#vid_1532&pid_1013&mi_02#collection".to_vec(),
            vendor_id: 5426,
            product_id: 4115,
            usage_page: 1,
            usage: 0,
            interface_number: -1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        }
    }

    #[test]
    fn controller_feature_interface_proof_rejects_another_container_and_interface() {
        let node = node();
        let mut interface = WindowsInterface {
            vendor_id: 5426,
            product_id: 4115,
            claim_interface: Some(2),
            feature_report_bytes: Some(91),
            device_container_id: Some("{DEVICE-A}".into()),
            path: String::from_utf8(node.path.clone()).unwrap(),
        };
        assert!(interface.matches(&node, 4115, "{device-a}", 2));
        assert!(!interface.matches(&node, 4115, "{DEVICE-B}", 2));
        assert!(!interface.matches(&node, 4115, "{DEVICE-A}", 1));
        assert!(!interface.matches(&node, 2636, "{DEVICE-A}", 2));
        interface.feature_report_bytes = Some(90);
        assert!(!interface.matches(&node, 4115, "{DEVICE-A}", 2));
        interface.feature_report_bytes = Some(91);
        interface.device_container_id = None;
        assert!(!interface.matches(&node, 4115, "{DEVICE-A}", 2));
    }

    #[test]
    fn controller_feature_missing_collection_observation_is_an_error() {
        assert!(serde_json::from_str::<NodesReply>("{}").is_err());
        assert!(serde_json::from_str::<NodesReply>(r#"{"nodes":null}"#).is_err());
    }
}

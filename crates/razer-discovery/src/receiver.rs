//! Retained receiver query routes. Collection scope is an observation key,
//! never a Windows ContainerId or a radio peer identity.
use crate::{direct, discovery};
use anyhow::{Context as _, ensure};
use razer_device::{backend::HidNode, receiver_capabilities};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_model::model::DeviceConnectionObservation;

#[derive(Clone, PartialEq, Eq)]
pub enum ReceiverRoute {
    Collection(HidNode),
    #[cfg(windows)]
    Container {
        product_id: u32,
        container: String,
    },
}

impl ReceiverRoute {
    /// Requires the receiver's own observed interface, not one of its peers.
    pub fn from_observation(observed: &discovery::ObservedDevice) -> anyhow::Result<Self> {
        ensure!(
            observed.peer_product_id().is_none()
                && matches!(
                    observed.connection(),
                    DeviceConnectionObservation::UsbPresent
                        | DeviceConnectionObservation::HidPresent
                ),
            "接收器查询需要本次发现的物理接口"
        );
        Self::physical_route(observed)
    }

    /// A receiver reply retains its exact physical collection/ContainerId.
    /// Only a currently connected peer can forward a source756 slave request
    /// to that owner; every command/query refreshes the actual owner interfaces.
    pub fn from_connected_peer(observed: &discovery::ObservedDevice) -> anyhow::Result<Self> {
        ensure!(
            observed.peer_product_id().is_some()
                && observed.connection() == DeviceConnectionObservation::ReceiverPeer(1),
            "DualLink slave 没有当前连接观察"
        );
        Self::physical_route(observed)
    }

    fn physical_route(observed: &discovery::ObservedDevice) -> anyhow::Result<Self> {
        let pid = u16::try_from(observed.physical_product_id())?;
        let cap = receiver_capabilities::capability(pid).context("接收器查询能力未核实")?;
        if let Some(node) = observed.hid_node() {
            ensure!(
                node.product_id == pid
                    && node.vendor_id == cap.vendor_id
                    && node.interface_number == i32::from(cap.claim_interface)
                    && direct::collection_scope(node) == observed.container(),
                "接收器 collection 与当前观察不匹配"
            );
            return Ok(Self::Collection(node.clone()));
        }
        #[cfg(windows)]
        {
            let container = observed.container();
            ensure!(
                container.len() == 38
                    && container.starts_with('{')
                    && container.ends_with('}')
                    && uuid::Uuid::parse_str(container).is_ok_and(|id| !id.is_nil()),
                "接收器 ContainerId 格式无效"
            );
            Ok(Self::Container {
                product_id: u32::from(pid),
                container: container.into(),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = cap;
            anyhow::bail!("此平台接收器观察缺少实际 HID collection")
        }
    }

    pub fn matches(&self, observed: &discovery::ObservedDevice) -> bool {
        Self::from_observation(observed)
            .or_else(|_| Self::from_connected_peer(observed))
            .is_ok_and(|current| current == *self)
    }

    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection(node) => u32::from(node.product_id),
            #[cfg(windows)]
            Self::Container { product_id, .. } => *product_id,
        }
    }

    pub fn query_for_source(
        &self,
        client: &mut ServiceClient,
        source_product_id: Option<u16>,
    ) -> anyhow::Result<discovery::ReceiverQueryProjection> {
        if source_product_id.is_none() {
            return self.query(client);
        }
        match self {
            #[cfg(windows)]
            Self::Container {
                product_id,
                container,
            } => {
                let snapshot = client.request(ServiceRequest::HidDevices)?;
                let path = discovery::source_receiver_path(
                    &snapshot,
                    container,
                    *product_id,
                    source_product_id,
                )?;
                let reply = client.request(ServiceRequest::ReceiverWirelessStatus {
                    path,
                    device_container_id: container.clone(),
                    source_product_id,
                })?;
                ensure!(
                    reply["source_product_id"].as_u64() == source_product_id.map(u64::from),
                    "接收器响应属于另一源产品 caller"
                );
                discovery::project_receiver_query(*product_id, container, &reply)
            }
            Self::Collection(node) => {
                query_collection_for_source(node, source_product_id, &mut |request| {
                    client.request(request)
                })
            }
        }
    }

    pub fn dock_mode(&self, client: &mut ServiceClient) -> anyhow::Result<bool> {
        match self {
            Self::Collection(node) => {
                let cap = razer_device::receiver_dock_mode::capability(node.product_id)?;
                let reply = client.request(ServiceRequest::HidNodes)?;
                let nodes: Vec<HidNode> = serde_json::from_value(reply["nodes"].clone())?;
                ensure!(
                    nodes.iter().filter(|candidate| *candidate == node).count() == 1,
                    "Dock mode collection is absent or ambiguous"
                );
                direct::source_reports_match(client, node, cap.report_bytes)?;
                let reply = client
                    .request(ServiceRequest::HidNodeReceiverDockMode { node: node.clone() })?;
                ensure!(
                    reply["node"] == serde_json::to_value(node)?
                        && reply["product_id"] == node.product_id
                        && reply["method"] == "getDockExtendedFirmwareVersion"
                        && reply["is_dongle"] == true,
                    "Dock mode reply belongs to another collection or method"
                );
                Ok(true)
            }
            #[cfg(windows)]
            Self::Container {
                product_id,
                container,
            } => {
                let hid = client.request(ServiceRequest::HidDevices)?;
                discovery::query_receiver_dock_mode(client, &hid, container, *product_id)
            }
        }
    }

    /// Refresh enumeration for each query; a retained UI identity is not proof
    /// that the same interface is still available. Worker rechecks around I/O.
    pub fn query(
        &self,
        client: &mut ServiceClient,
    ) -> anyhow::Result<discovery::ReceiverQueryProjection> {
        match self {
            Self::Collection(node) => {
                query_collection_for_source(node, None, &mut |request| client.request(request))
            }
            #[cfg(windows)]
            Self::Container {
                product_id,
                container,
            } => {
                let hid = client.request(ServiceRequest::HidDevices)?;
                let reply = discovery::query_receiver(client, &hid, container, *product_id)?;
                discovery::project_receiver_query(*product_id, container, &reply)
            }
        }
    }
}

fn query_collection_for_source(
    node: &HidNode,
    source_product_id: Option<u16>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
) -> anyhow::Result<discovery::ReceiverQueryProjection> {
    let cap = receiver_capabilities::for_source(node.product_id, source_product_id)?;
    ensure!(
        node.vendor_id == cap.vendor_id && node.interface_number == i32::from(cap.claim_interface),
        "接收器 collection 与源 factory 接口不匹配"
    );
    let reply = request(ServiceRequest::HidNodes)?;
    let nodes: Vec<HidNode> = serde_json::from_value(reply["nodes"].clone())?;
    ensure!(
        nodes.iter().filter(|candidate| *candidate == node).count() == 1,
        "接收器 collection 已变化或不唯一"
    );
    direct::source_reports_match_with(node, cap.report_bytes, request)?;
    let reply = request(match source_product_id {
        Some(source_product_id) => ServiceRequest::HidNodeReceiverStatusForSource {
            node: node.clone(),
            source_product_id,
        },
        None => ServiceRequest::HidNodeReceiverStatus { node: node.clone() },
    })?;
    ensure!(
        source_product_id.is_none() && reply["source_product_id"].is_null()
            || reply["source_product_id"].as_u64() == Some(u64::from(cap.source_product_id)),
        "接收器响应属于另一源产品 caller"
    );
    discovery::project_hid_receiver_query(node, &reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn node(pid: u16) -> HidNode {
        let cap = receiver_capabilities::capability(pid).unwrap();
        HidNode {
            path: b"simulated-source-receiver".to_vec(),
            vendor_id: cap.vendor_id,
            product_id: pid,
            interface_number: i32::from(cap.claim_interface),
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        }
    }

    fn reply(node: &HidNode) -> Value {
        json!({"node":node,"identity_scope":"hid_collection","query":"receiver_wireless_status_v2",
            "source_product_id":756,"source_class":"rzDevice25DualLinkKeyboard",
            "device_count":1,"devices":[{"product_id":756,"status":1}]})
    }

    #[test]
    fn source_keyboard_queries_preserve_the_selected_caller_on_every_audited_master() {
        for pid in [164, 179, 241, 757] {
            let node = node(pid);
            let mut calls = 0;
            let projection = query_collection_for_source(&node, Some(756), &mut |request| {
                calls += 1;
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { node: requested } => {
                        assert_eq!(requested, node);
                        json!({"node":node,"reports":{"feature":{"9":91}}})
                    }
                    ServiceRequest::HidNodeReceiverStatusForSource {
                        node: requested,
                        source_product_id,
                    } => {
                        assert_eq!(requested, node);
                        assert_eq!(source_product_id, 756);
                        reply(&node)
                    }
                    _ => panic!("unexpected source route"),
                })
            })
            .unwrap();
            assert_eq!(calls, 3);
            assert_eq!(projection.devices().len(), 1);
            let peer = &projection.devices()[0];
            assert_eq!(peer.product_id(), 756);
            assert_eq!(peer.peer_product_id(), Some(756));
            assert_eq!(peer.physical_product_id(), u32::from(pid));
            assert_eq!(peer.hid_node(), Some(&node));
        }
    }

    #[test]
    fn stale_collection_and_wrong_source_reply_never_project_a_peer() {
        let node = node(241);
        for mode in 0..5 {
            let result = query_collection_for_source(&node, Some(756), &mut |request| {
                Ok(match request {
                    ServiceRequest::HidNodes if mode == 0 => json!({"nodes":[]}),
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { .. } => {
                        json!({"node":node,"reports":{"feature":{"3":91}}})
                    }
                    ServiceRequest::HidNodeReceiverStatusForSource { .. } => {
                        let mut value = reply(&node);
                        match mode {
                            1 => value["source_product_id"] = json!(241),
                            2 => value["source_class"] = json!("rzDevice25"),
                            3 => value["node"]["path"] = json!([1]),
                            _ => value["source_product_id"] = Value::Null,
                        }
                        value
                    }
                    _ => panic!("unexpected source route"),
                })
            });
            assert!(result.is_err(), "mode {mode}");
        }
        assert!(
            query_collection_for_source(&node, Some(60000), &mut |_| panic!(
                "unreviewed source must not query"
            ))
            .is_err()
        );
    }
}

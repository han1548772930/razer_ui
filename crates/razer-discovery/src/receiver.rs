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
        Self::from_observation(observed).is_ok_and(|current| current == *self)
    }

    /// Refresh enumeration for each query; a retained UI identity is not proof
    /// that the same interface is still available. Worker rechecks around I/O.
    pub fn query(
        &self,
        client: &mut ServiceClient,
    ) -> anyhow::Result<discovery::ReceiverQueryProjection> {
        match self {
            Self::Collection(node) => {
                let cap = receiver_capabilities::capability(node.product_id)
                    .context("接收器查询能力未核实")?;
                let reply = client.request(ServiceRequest::HidNodes)?;
                let nodes: Vec<HidNode> = serde_json::from_value(reply["nodes"].clone())
                    .context("HID 节点响应格式无效")?;
                ensure!(
                    nodes.iter().filter(|candidate| *candidate == node).count() == 1,
                    "接收器 collection 已变化或不唯一"
                );
                direct::reports_match(client, node, cap.report_id, cap.report_bytes)?;
                let reply =
                    client.request(ServiceRequest::HidNodeReceiverStatus { node: node.clone() })?;
                discovery::project_hid_receiver_query(node, &reply)
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

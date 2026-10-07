//! Pure projection of a verified V2 reply into UI bindings and scoped peers.
//! No host publisher, localStorage mutation, service request or device I/O.
use super::{DiscoverySnapshot, ObservedDevice, ObservedTransport, receiver_peers};
use crate::backend::{
    device_identity::{self, IdentityLookup},
    receiver_capabilities::{self, ReceiverCapability},
    receiver_catalog,
};
use crate::model::DeviceConnectionObservation;
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[cfg(test)]
#[path = "discovery_receiver_projection_tests.rs"]
mod tests;

#[derive(Clone)]
pub(crate) struct ReceiverQueryProjection {
    receiver_product_id: u32,
    container: String,
    snapshot: DiscoverySnapshot,
    pairing: Result<Value, String>,
}
impl ReceiverQueryProjection {
    pub(crate) fn receiver_product_id(&self) -> u32 {
        self.receiver_product_id
    }
    pub(crate) fn container(&self) -> &str {
        &self.container
    }
    pub(crate) fn devices(&self) -> &[ObservedDevice] {
        self.snapshot.devices()
    }
    pub(crate) fn errors(&self) -> &[String] {
        self.snapshot.errors()
    }
    /// A binding-description failure must not erase unrelated known peers.
    pub(crate) fn pairing_payload(&self) -> anyhow::Result<Value> {
        self.pairing.clone().map_err(anyhow::Error::msg)
    }
    pub(super) fn into_snapshot(self) -> DiscoverySnapshot {
        self.snapshot
    }
}

struct MappedPeer {
    product_id: u32,
    raw_product_id: u32,
    status: u8,
}
struct PeerProjection {
    peers: Vec<MappedPeer>,
    errors: Vec<String>,
}

fn capability(receiver_pid: u32) -> anyhow::Result<&'static ReceiverCapability> {
    u16::try_from(receiver_pid)
        .ok()
        .and_then(receiver_capabilities::capability)
        .context("接收器查询能力未核实")
}

/// Use the same source catalog relation for startup, subsequent queries and UI.
/// A product's own dongle can report its peer using that dongle PID. A standalone
/// receiver's own row is not a peer; sentinel and self rows never create devices.
fn project_peers(receiver_pid: u32, value: &Value) -> anyhow::Result<PeerProjection> {
    let capability = capability(receiver_pid)?;
    let own_dongle = matches!(
        device_identity::lookup(receiver_pid),
        IdentityLookup::Unique(identity) if identity.is_dongle
    );
    let mut projection = PeerProjection {
        peers: Vec::new(),
        errors: Vec::new(),
    };
    let mut ambiguous_products = BTreeSet::new();
    for (raw_pid, status) in receiver_peers(value)? {
        if raw_pid == 65535 || (raw_pid == receiver_pid && !own_dongle) {
            continue;
        }
        let product_id = match device_identity::lookup_receiver_peer(
            raw_pid,
            capability.peer_match_product_id,
        ) {
            IdentityLookup::Unique(identity) => identity.product_id,
            IdentityLookup::Unmatched { .. } => {
                projection
                    .errors
                    .push(format!("查询返回 PID {raw_pid}，官方目录无对应产品"));
                continue;
            }
            IdentityLookup::Ambiguous { candidates, .. } => {
                projection.errors.push(format!(
                    "查询 PID {raw_pid} 对应 {} 个产品，尚需身份读取消除歧义",
                    candidates.len()
                ));
                continue;
            }
        };
        // Distinct raw rows may normalize to the same product in a keyboard
        // query. Do not let insertion order silently choose one radio status.
        if projection
            .peers
            .iter()
            .any(|peer| peer.product_id == product_id)
        {
            ambiguous_products.insert(product_id);
            projection.errors.push(format!(
                "多个接收器条目映射到产品 {product_id}，无法区分同一容器内的设备身份"
            ));
        }
        projection.peers.push(MappedPeer {
            product_id,
            raw_product_id: raw_pid,
            status,
        });
    }
    projection
        .peers
        .retain(|peer| !ambiguous_products.contains(&peer.product_id));
    Ok(projection)
}

fn payload(projection: &PeerProjection) -> anyhow::Result<Value> {
    ensure!(
        projection.errors.is_empty(),
        "{}",
        projection.errors.join("；")
    );
    let mut rows = Vec::new();
    for peer in &projection.peers {
        let product =
            crate::product::registered(peer.product_id).context("配对产品尚无本地页面")?;
        let categories = product.categories();
        let description = receiver_catalog::description(peer.raw_product_id);
        let category = if let Some(description) = description {
            description.category()
        } else if categories.contains(&"MOUSE") {
            "MOUSE"
        } else if categories.contains(&"KEYBOARD") {
            "KEYBOARD"
        } else {
            anyhow::bail!(
                "配对 PID {} 的产品类别不适用于底座页面",
                peer.raw_product_id
            )
        };
        ensure!(
            matches!(category, "MOUSE" | "KEYBOARD"),
            "配对设备类别不适用于此页面"
        );
        let product_name = description
            .map(|description| json!(description.product_name()))
            .unwrap_or_else(|| json!({"en":product.name()}));
        rows.push(json!({
            "productId":peer.product_id,"dongleId":peer.raw_product_id,
            "status":peer.status,"category":category,"productName":product_name
        }));
    }
    Ok(Value::Array(rows))
}

/// Minimal DUALLINK_BIND_INFO for callers already owning a validated query.
/// Edition/layout/serial are absent because this protocol does not return them.
pub(crate) fn pairing_payload(receiver_pid: u32, value: &Value) -> anyhow::Result<Value> {
    payload(&project_peers(receiver_pid, value)?)
}

/// Scope-check a raw worker reply without modifying it, then project its rows.
/// Snapshot errors mean partial identity knowledge: callers must preserve known
/// peers and must not infer disconnection from missing unresolved identities.
pub(crate) fn project_receiver_query(
    receiver_pid: u32,
    container: &str,
    value: &Value,
) -> anyhow::Result<ReceiverQueryProjection> {
    let capability = capability(receiver_pid)?;
    let valid_container = container
        .strip_prefix('{')
        .and_then(|container| container.strip_suffix('}'))
        .is_some_and(|inner| {
            inner.len() == 36 && uuid::Uuid::parse_str(inner).is_ok_and(|id| !id.is_nil())
        });
    ensure!(valid_container, "接收器 ContainerId 格式无效");
    ensure!(
        value["query"] == "receiver_wireless_status_v2"
            && value["vendor_id"] == capability.vendor_id
            && value["product_id"] == receiver_pid
            && value["claim_interface"] == capability.claim_interface
            && value["report_bytes"] == capability.report_bytes
            && value["device_container_id"]
                .as_str()
                .is_some_and(|id| id.eq_ignore_ascii_case(container)),
        "接收器查询响应与请求作用域不匹配"
    );
    ensure!(
        ["path", "device_instance_id"].iter().all(|field| {
            value[*field]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty())
        }),
        "接收器响应缺少实际接口身份"
    );
    ensure!(
        value["devices"]
            .as_array()
            .is_some_and(|rows| value["device_count"].as_u64() == Some(rows.len() as u64)),
        "接收器响应设备数量不匹配"
    );
    let projection = project_peers(receiver_pid, value)?;
    let pairing = payload(&projection).map_err(|error| format!("{error:#}"));
    let snapshot = DiscoverySnapshot {
        devices: projection
            .peers
            .into_iter()
            .map(|peer| ObservedDevice {
                product_id: peer.product_id,
                real_product_id: peer.raw_product_id,
                container: container.to_string(),
                serial: String::new(),
                use_ble: false,
                transport: Some(ObservedTransport::Dongle),
                connection: DeviceConnectionObservation::ReceiverPeer(peer.status),
                physical_product_id: receiver_pid,
                peer_product_id: Some(peer.raw_product_id),
                read_values: None,
            })
            .collect(),
        errors: projection.errors,
    };
    Ok(ReceiverQueryProjection {
        receiver_product_id: receiver_pid,
        container: container.to_string(),
        snapshot,
        pairing,
    })
}

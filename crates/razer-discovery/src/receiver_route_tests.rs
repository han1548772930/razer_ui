//! Scope and reply regressions compiled only under the development policy.
use super::*;
use crate::receiver::ReceiverRoute;
use razer_device::backend::HidNode;
use serde_json::json;

fn owner() -> ObservedDevice {
    let cap = razer_device::receiver_capabilities::capability(179).unwrap();
    let node = HidNode {
        path: b"observed-receiver".to_vec(),
        vendor_id: cap.vendor_id,
        product_id: 179,
        interface_number: i32::from(cap.claim_interface),
        usage_page: 0xff00,
        usage: 1,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    ObservedDevice {
        product_id: 179,
        real_product_id: 179,
        physical_product_id: 179,
        peer_product_id: None,
        container: crate::direct::collection_scope(&node),
        hid_node: Some(node),
        serial: String::new(),
        use_ble: false,
        transport: None,
        connection: DeviceConnectionObservation::HidPresent,
        read_values: None,
    }
}

#[test]
fn retained_receiver_route_rejects_peer_unknown_connection_and_replaced_collection() {
    let mut observed = owner();
    let route = ReceiverRoute::from_observation(&observed).unwrap();
    assert!(route.matches(&observed));
    observed.hid_node.as_mut().unwrap().path.push(7);
    observed.container = crate::direct::collection_scope(observed.hid_node.as_ref().unwrap());
    assert!(!route.matches(&observed));
    observed = owner();
    observed.peer_product_id = Some(183);
    assert!(ReceiverRoute::from_observation(&observed).is_err());
    observed.peer_product_id = None;
    observed.connection = DeviceConnectionObservation::ReceiverPeer(1);
    assert!(ReceiverRoute::from_observation(&observed).is_err());
    observed = owner();
    observed.hid_node.as_mut().unwrap().interface_number = -1;
    assert!(ReceiverRoute::from_observation(&observed).is_err());
}

#[test]
fn collection_binding_projection_preserves_raw_identity_and_checks_reply_scope() {
    let observed = owner();
    let node = observed.hid_node.as_ref().unwrap();
    let cap = razer_device::receiver_capabilities::capability(179).unwrap();
    let reply = json!({"node":node,"identity_scope":"hid_collection",
        "query":"receiver_wireless_status_v2","source_class":cap.source_class,
        "device_count":1,"devices":[{"product_id":183,"status":1}]});
    let projection = project_hid_receiver_query(node, &reply).unwrap();
    assert_eq!(projection.container(), observed.container());
    let peer = &projection.devices()[0];
    assert_eq!(peer.product_id(), 182);
    assert_eq!(peer.peer_product_id(), Some(183));
    assert_eq!(peer.hid_node(), Some(node));
    assert_eq!(
        peer.connection(),
        DeviceConnectionObservation::ReceiverPeer(1)
    );
    let mut wrong = reply.clone();
    wrong["node"]["path"] = json!([0]);
    assert!(project_hid_receiver_query(node, &wrong).is_err());
    wrong = reply.clone();
    wrong["identity_scope"] = json!("windows_container");
    assert!(project_hid_receiver_query(node, &wrong).is_err());
    wrong = reply;
    wrong["device_count"] = json!(0);
    assert!(project_hid_receiver_query(node, &wrong).is_err());
}

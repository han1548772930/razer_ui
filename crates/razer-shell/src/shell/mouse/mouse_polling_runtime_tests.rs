use super::{PollingField, State};
use razer_device::{backend::HidNode, receiver_capabilities};
use razer_discovery::discovery::{self, ObservedDevice};
use serde_json::json;

fn observation(path: &[u8], peer: u16, status: u8) -> ObservedDevice {
    let source = receiver_capabilities::capability(241).unwrap();
    let node = HidNode {
        path: path.to_vec(),
        vendor_id: source.vendor_id,
        product_id: 241,
        interface_number: i32::from(source.claim_interface),
        usage_page: 0xff00,
        usage: 1,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    project(&node, peer, status)
}

fn project(node: &HidNode, peer: u16, status: u8) -> ObservedDevice {
    let source = receiver_capabilities::capability(node.product_id).unwrap();
    let projection = discovery::project_hid_receiver_query(
        node,
        &json!({
            "node":node, "identity_scope":"hid_collection", "query":"receiver_wireless_status_v2",
            "source_class":source.source_class, "device_count":1,
            "devices":[{"product_id":peer,"status":status}]
        }),
    )
    .unwrap();
    assert!(projection.errors().is_empty(), "{:?}", projection.errors());
    let [observed] = projection.devices() else {
        panic!("one actual receiver peer identity expected");
    };
    observed.clone()
}

#[test]
fn accepted_work_is_pending_before_the_executor_polls_it() {
    let mut state = State::default();
    let (first, accepted) = state.accept().unwrap();
    let (second, other) = state.accept().unwrap();
    assert_ne!(first, second);
    assert!(state.pending());
    drop(accepted);
    assert!(state.pending());
    drop(other);
    assert!(!state.pending());
}

#[test]
fn dropped_unpolled_delivery_releases_its_accepted_work() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    let unpolled = async move {
        let _accepted = accepted;
        panic!("canceled polling future must never be polled");
    };
    assert!(state.pending());
    drop(unpolled);
    assert!(!state.pending());
}

#[test]
fn shutdown_closes_admission_while_accepted_work_remains_pending() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    assert!(state.pending());
    state.resume_after_exit();
    assert!(state.accept().is_none());
    drop(accepted);
    assert!(!state.pending());
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    state.resume_after_exit();
    assert!(state.accept().is_some());
}

#[test]
fn exact_route_survives_repeated_discovery_and_unrelated_device_changes() {
    let route = observation(b"mouse-first", 183, 1);
    let device = route.clone().into_device().unwrap();
    let repeated = observation(b"mouse-first", 183, 1);
    assert!(super::current_route(
        &route,
        std::slice::from_ref(&repeated),
        &device,
        PollingField::Wireless,
    ));
    let unrelated = observation(b"other-receiver", 227, 1);
    assert!(super::current_route(
        &route,
        &[unrelated, repeated],
        &device,
        PollingField::Wireless,
    ));
}

#[test]
fn disconnect_replacement_peer_change_and_ambiguity_reject_old_readback() {
    let route = observation(b"mouse-first", 183, 1);
    let device = route.clone().into_device().unwrap();
    for current in [
        vec![],
        vec![observation(b"mouse-first", 183, 0)],
        vec![observation(b"mouse-replaced", 183, 1)],
        vec![observation(b"mouse-first", 227, 1)],
        vec![route.clone(), route.clone()],
    ] {
        assert!(!super::current_route(
            &route,
            &current,
            &device,
            PollingField::Wireless,
        ));
    }
}

#[test]
fn matching_device_identity_does_not_authorize_a_different_polling_transport() {
    let route = observation(b"mouse-first", 183, 1);
    let device = route.clone().into_device().unwrap();
    assert!(super::supports_route(&route, PollingField::Wireless));
    assert!(!super::supports_route(&route, PollingField::Wired));
    assert!(!super::current_route(
        &route,
        std::slice::from_ref(&route),
        &device,
        PollingField::Wired,
    ));
}

#[test]
fn disconnected_receiver_never_authorizes_a_new_wireless_submission() {
    let route = observation(b"mouse-first", 183, 0);
    assert!(!super::supports_route(&route, PollingField::Wireless));
    assert!(!super::supports_route(&route, PollingField::Wired));
}

#[test]
fn full_hid_collection_identity_is_required_even_when_transport_and_path_match() {
    let route = observation(b"mouse-first", 183, 1);
    let device = route.clone().into_device().unwrap();
    let mut replaced = route.hid_node().unwrap().clone();
    replaced.release_number = 0x100;
    let replaced = project(&replaced, 183, 1);
    assert!(replaced.matches(&device));
    assert_eq!(replaced.transport(), route.transport());
    assert!(!super::current_route(
        &route,
        &[replaced],
        &device,
        PollingField::Wireless,
    ));
}

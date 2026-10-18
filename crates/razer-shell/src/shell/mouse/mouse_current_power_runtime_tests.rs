use super::{Request, RouteKind, State};
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
fn dropped_unpolled_delivery_releases_admission_without_starting_a_service() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    let unpolled = async move {
        let _accepted = accepted;
        panic!("canceled power future must never be polled");
    };
    assert!(state.pending());
    drop(unpolled);
    assert!(!state.pending());
}

#[test]
fn shutdown_stops_admission_before_waiting_for_accepted_work() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    assert!(state.pending());
    drop(accepted);
    assert!(!state.pending());
}

#[test]
fn idle_and_current_power_keep_the_same_route_across_unrelated_refreshes() {
    for (peer, kind) in [(183, RouteKind::Idle), (227, RouteKind::Current)] {
        let route = observation(b"power-owner", peer, 1);
        let device = route.clone().into_device().unwrap();
        let repeated = observation(b"power-owner", peer, 1);
        assert!(super::current_route(
            &route,
            std::slice::from_ref(&repeated),
            &device,
            kind
        ));
        let unrelated = observation(b"unrelated-receiver", 183, 1);
        assert!(super::current_route(
            &route,
            &[unrelated, repeated],
            &device,
            kind
        ));
    }
}

#[test]
fn replacement_disconnect_peer_change_and_ambiguity_reject_old_power_results() {
    for (peer, kind) in [(183, RouteKind::Idle), (227, RouteKind::Current)] {
        let route = observation(b"power-owner", peer, 1);
        let device = route.clone().into_device().unwrap();
        let other_peer = if peer == 183 { 227 } else { 183 };
        for current in [
            vec![],
            vec![observation(b"power-owner", peer, 0)],
            vec![observation(b"power-replaced", peer, 1)],
            vec![observation(b"power-owner", other_peer, 1)],
            vec![route.clone(), route.clone()],
        ] {
            assert!(!super::current_route(&route, &current, &device, kind));
        }
    }
}

#[test]
fn a_matching_transport_and_path_do_not_replace_the_complete_collection_identity() {
    for (peer, kind) in [(183, RouteKind::Idle), (227, RouteKind::Current)] {
        let route = observation(b"power-owner", peer, 1);
        let device = route.clone().into_device().unwrap();
        let mut node = route.hid_node().unwrap().clone();
        node.release_number = 0x100;
        let replaced = project(&node, peer, 1);
        assert!(replaced.matches(&device));
        assert_eq!(replaced.transport(), route.transport());
        assert!(!super::current_route(&route, &[replaced], &device, kind));
    }
}

#[test]
fn idle_selection_rejects_ambiguous_or_offline_routes_before_scheduling() {
    let request = Request::Idle {
        scope: super::MousePollingScope::new(super::EntityId::from(1), 2, 3),
        minutes: 7,
        valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
    };
    let route = observation(b"power-owner", 183, 1);
    let device = route.clone().into_device().unwrap();
    assert!(super::select_route(std::slice::from_ref(&route), &device, &request).is_some());
    assert!(super::select_route(&[route.clone(), route], &device, &request).is_none());
    assert!(
        super::select_route(&[observation(b"power-owner", 183, 0)], &device, &request).is_none()
    );
}

#[test]
fn current_and_legacy_requests_keep_their_source_capability_boundaries() {
    let idle = observation(b"power-owner", 183, 1);
    let current = observation(b"power-owner", 227, 1);
    assert!(super::supports_route(&idle, RouteKind::Idle));
    assert!(!super::supports_route(&idle, RouteKind::Current));
    assert!(super::supports_route(&current, RouteKind::Current));
    assert!(!super::supports_route(&current, RouteKind::Idle));
}

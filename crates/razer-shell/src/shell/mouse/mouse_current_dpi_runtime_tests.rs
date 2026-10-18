use super::State;
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
    discovery::project_hid_receiver_query(
        &node,
        &json!({
            "node":node, "identity_scope":"hid_collection", "query":"receiver_wireless_status_v2",
            "source_class":source.source_class, "device_count":1,
            "devices":[{"product_id":peer,"status":status}]
        }),
    )
    .unwrap()
    .devices()[0]
        .clone()
}

#[test]
fn accepted_work_is_pending_before_the_executor_polls_it() {
    let mut state = State::default();
    let (first, guard) = state.accept().unwrap();
    let (second, other) = state.accept().unwrap();
    assert_ne!(first, second);
    assert!(state.pending());
    drop(guard);
    assert!(state.pending());
    drop(other);
    assert!(!state.pending());
}

#[test]
fn dropped_scheduling_releases_admission_without_creating_a_service() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    let unpolled = async move {
        let _accepted = accepted;
        panic!("canceled DPI future must never be polled");
    };
    assert!(state.pending());
    drop(unpolled);
    assert!(!state.pending());
}

#[test]
fn shutdown_closes_admission_before_waiting_for_deliveries() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    assert!(state.pending());
    drop(accepted);
    assert!(!state.pending());
}

#[test]
fn ordinary_discovery_refresh_preserves_the_exact_mouse_route() {
    let route = observation(b"mouse-first", 182, 1);
    let device = route.clone().into_device().unwrap();
    let repeated = observation(b"mouse-first", 182, 1);
    assert!(super::current_route(&route, &[repeated.clone()], &device));
    let unrelated = observation(b"keyboard", 656, 1);
    assert!(super::current_route(
        &route,
        &[unrelated, repeated],
        &device
    ));
}

#[test]
fn replacement_disconnect_ambiguity_and_peer_change_reject_old_dpi_readback() {
    let route = observation(b"mouse-first", 182, 1);
    let device = route.clone().into_device().unwrap();
    for next in [
        vec![],
        vec![observation(b"mouse-first", 182, 0)],
        vec![observation(b"mouse-replaced", 182, 1)],
        vec![observation(b"mouse-first", 226, 1)],
        vec![route.clone(), route.clone()],
    ] {
        assert!(!super::current_route(&route, &next, &device));
    }
}

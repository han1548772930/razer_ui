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
            "node":node,
            "identity_scope":"hid_collection",
            "query":"receiver_wireless_status_v2",
            "source_class":source.source_class,
            "device_count":1,
            "devices":[{"product_id":peer,"status":status}]
        }),
    )
    .unwrap()
    .devices()[0]
        .clone()
}

#[test]
fn accepted_work_is_pending_before_its_background_future_is_polled() {
    let mut state = State::default();
    assert!(!state.pending());
    let (first, accepted_first) = state.accept().unwrap();
    let (second, accepted_second) = state.accept().unwrap();
    assert_ne!(first, second);
    assert!(state.pending());
    drop(accepted_first);
    assert!(state.pending());
    drop(accepted_second);
    assert!(!state.pending());
}

#[test]
fn canceled_scheduling_releases_the_accepted_operation_without_starting_io() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    let unpolled = async move {
        let _accepted = accepted;
        panic!("The canceled background future must never be polled");
    };
    assert!(state.pending());
    drop(unpolled);
    assert!(!state.pending());
}

#[test]
fn shutdown_closes_admission_before_any_window_or_request_exists() {
    let mut state = State::default();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    assert!(!state.pending());
    assert!(state.take_for_shutdown().is_empty());
    state.resume_after_exit();
    let (_, accepted) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    state.resume_after_exit();
    assert!(state.accept().is_none());
    drop(accepted);
    state.resume_after_exit();
    assert!(state.accept().is_some());
}

#[test]
fn repeated_discovery_preserves_the_same_complete_keyboard_connection() {
    let route = observation(b"keyboard-first", 656, 1);
    let device = route.clone().into_device().unwrap();
    let repeat = observation(b"keyboard-first", 656, 1);
    assert!(super::current_route(&route, &[repeat.clone()], &device));
    let unrelated = observation(b"keyboard-second", 662, 1);
    assert!(super::current_route(&route, &[unrelated, repeat], &device));
}

#[test]
fn route_replacement_disconnect_ambiguity_and_peer_changes_reject_the_old_request() {
    let route = observation(b"keyboard-first", 656, 1);
    let device = route.clone().into_device().unwrap();
    let offline = observation(b"keyboard-first", 656, 0);
    let replaced = observation(b"keyboard-replaced", 656, 1);
    let other_peer = observation(b"keyboard-first", 662, 1);
    assert!(!super::current_route(&route, &[offline], &device));
    assert!(!super::current_route(&route, &[replaced], &device));
    assert!(!super::current_route(&route, &[other_peer], &device));
    assert!(!super::current_route(&route, &[], &device));
    assert!(!super::current_route(
        &route,
        &[route.clone(), route.clone()],
        &device
    ));
}

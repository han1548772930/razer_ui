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
    let projection = discovery::project_hid_receiver_query(
        &node,
        &json!({
            "node":node,"identity_scope":"hid_collection","query":"receiver_wireless_status_v2",
            "source_class":source.source_class,"device_count":1,
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
fn mouse_scroll_accepted_work_is_pending_before_poll_and_until_each_worker_releases() {
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
fn mouse_scroll_unpolled_scheduling_releases_admission_without_opening_a_device() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    let unpolled = async move {
        let _accepted = accepted;
        panic!("canceled scroll future must never be polled");
    };
    assert!(state.pending());
    drop(unpolled);
    assert!(!state.pending());
}

#[test]
fn mouse_scroll_shutdown_closes_admission_before_waiting_for_accepted_work() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    assert!(state.pending());
    state.resume_after_exit();
    assert!(state.accept().is_none());
    drop(accepted);
    assert!(!state.pending());
    state.resume_after_exit();
    assert!(state.accept().is_some());
}

#[test]
fn mouse_scroll_repeated_route_and_unrelated_discovery_preserve_accepted_work() {
    let route = observation(b"scroll-mouse-first", 227, 1);
    let device = route.clone().into_device().unwrap();
    let repeated = observation(b"scroll-mouse-first", 227, 1);
    assert!(super::current_route(&route, &[repeated.clone()], &device));
    let unrelated = observation(b"unrelated-mouse", 183, 1);
    assert!(super::current_route(
        &route,
        &[unrelated, repeated],
        &device
    ));
}

#[test]
fn mouse_scroll_disconnect_replacement_peer_change_and_ambiguity_reject_stale_readback() {
    let route = observation(b"scroll-mouse-first", 227, 1);
    let device = route.clone().into_device().unwrap();
    for next in [
        vec![],
        vec![observation(b"scroll-mouse-first", 227, 0)],
        vec![observation(b"scroll-mouse-replaced", 227, 1)],
        vec![observation(b"scroll-mouse-first", 183, 1)],
        vec![route.clone(), route.clone()],
    ] {
        assert!(!super::current_route(&route, &next, &device));
    }
}

use super::*;
use razer_device::receiver_capabilities;
use razer_discovery::discovery;
use serde_json::json;

#[::core::prelude::v1::test]
fn dpi_stage_admission_is_pending_before_any_worker_poll() {
    let mut state = State::default();
    let (first, first_guard) = state.accept().unwrap();
    let (second, second_guard) = state.accept().unwrap();
    assert_ne!(first, second);
    assert!(state.pending());
    drop(first_guard);
    assert!(state.pending());
    drop(second_guard);
    assert!(!state.pending());
}

#[::core::prelude::v1::test]
fn dpi_stage_canceled_scheduling_releases_admission_without_starting_service() {
    let mut state = State::default();
    let (_, guard) = state.accept().unwrap();
    let future = async move {
        let _guard = guard;
        panic!("canceled stage request must not execute");
    };
    assert!(state.pending());
    drop(future);
    assert!(!state.pending());
}

#[::core::prelude::v1::test]
fn dpi_stage_shutdown_rejects_new_work_and_waits_for_accepted_release() {
    let mut state = State::default();
    let (_, guard) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    assert!(state.pending());
    drop(guard);
    assert!(!state.pending());
}

#[::core::prelude::v1::test]
fn dpi_stage_exit_resume_reopens_admission_without_replaying_old_requests() {
    let mut state = State::default();
    let (first, accepted) = state.accept().unwrap();
    state.take_for_shutdown();
    state.resume_after_exit();
    assert!(state.accept().is_none());
    assert!(state.pending());
    drop(accepted);
    state.resume_after_exit();
    assert!(!state.pending());
    let (next, next_accepted) = state.accept().unwrap();
    assert!(next > first);
    assert!(state.operations.is_empty());
    drop(next_accepted);
    assert!(!state.pending());
}

fn receiver(container: &str, status: u8) -> discovery::ReceiverQueryProjection {
    let capability = receiver_capabilities::capability(179).unwrap();
    discovery::project_receiver_query(
        179,
        container,
        &json!({
            "query":"receiver_wireless_status_v2",
            "vendor_id":capability.vendor_id,
            "product_id":179,
            "claim_interface":capability.claim_interface,
            "report_bytes":capability.report_bytes,
            "path":"test-observation-path",
            "device_instance_id":"test-observation-instance",
            "device_container_id":container,
            "device_count":1,
            "devices":[{"product_id":183,"status":status}]
        }),
    )
    .unwrap()
}

#[::core::prelude::v1::test]
fn dpi_stage_routes_keep_unrelated_refresh_and_reject_disconnect_replacement_and_ambiguity() {
    let first = "{7A1F7C04-2B84-4BA3-A66C-17023B63E99F}";
    let other = "{7A1F7C04-2B84-4BA3-A66C-17023B63E990}";
    let connected = receiver(first, 1);
    let route = &connected.devices()[0];
    let device = route.clone().into_device().unwrap();
    assert!(current_route(route, connected.devices(), &device));
    let mut observations = connected.devices().to_vec();
    observations.extend_from_slice(receiver(other, 0).devices());
    assert!(current_route(route, &observations, &device));
    assert!(!current_route(route, receiver(first, 0).devices(), &device));
    assert!(!current_route(route, receiver(other, 1).devices(), &device));
    assert!(!current_route(route, &[], &device));
    assert!(!current_route(
        route,
        &[route.clone(), route.clone()],
        &device
    ));
}

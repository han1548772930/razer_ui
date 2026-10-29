use super::{
    PendingBleCatalog, State, catalog_for_verified_read, current_ble_completion,
    discard_stale_ble_catalog,
};
use razer_device::{backend::HidNode, mouse_current_dpi::DpiReport, receiver_capabilities};
use razer_discovery::discovery::{self, ObservedDevice};
use razer_model::model::{Device, Profile};
use razer_pages::features::mouse_polling::MousePollingScope;
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
    state.resume_after_exit();
    assert!(state.accept().is_none());
    drop(accepted);
    assert!(!state.pending());
    state.resume_after_exit();
    assert!(state.accept().is_some());
}

#[test]
fn ordinary_discovery_refresh_preserves_the_exact_mouse_route() {
    let route = observation(b"mouse-first", 183, 1);
    let device = route.clone().into_device().unwrap();
    let repeated = observation(b"mouse-first", 183, 1);
    assert!(super::current_route(&route, &[repeated.clone()], &device));
    let unrelated = observation(b"unrelated-mouse", 227, 1);
    assert!(super::current_route(
        &route,
        &[unrelated, repeated],
        &device
    ));
}

#[test]
fn replacement_disconnect_ambiguity_and_peer_change_reject_old_dpi_readback() {
    let route = observation(b"mouse-first", 183, 1);
    let device = route.clone().into_device().unwrap();
    for next in [
        vec![],
        vec![observation(b"mouse-first", 183, 0)],
        vec![observation(b"mouse-replaced", 183, 1)],
        vec![observation(b"mouse-first", 227, 1)],
        vec![route.clone(), route.clone()],
    ] {
        assert!(!super::current_route(&route, &next, &device));
    }
}

fn catalog_scope() -> MousePollingScope {
    MousePollingScope::new(super::EntityId::from(1), 2, 3)
}

fn catalog_baseline() -> Device {
    let mut device = razer_model::product_preview::registered_preview(226).unwrap();
    device.device_container_id = "current-ble-container".into();
    device.serial_number.clear();
    device.active_profile = "local-first".into();
    device.profiles = ["first", "second"]
        .map(|guid| Profile {
            id: format!("local-{guid}"),
            guid: guid.into(),
            name: guid.into(),
            source_settings: Some(json!({"guid":guid,"name":guid})),
            settings: None,
            dpi_stages: None,
        })
        .into();
    device.source_device_settings = Some(json!({"version":7}));
    device
}

fn incoming_catalog(baseline: &Device) -> PendingBleCatalog<&'static str> {
    PendingBleCatalog {
        serial: "actual-ble-serial".into(),
        catalog: "coherent current-serial catalog",
        baseline: baseline.clone(),
        scope: catalog_scope(),
    }
}

fn read_result(verified: bool) -> Result<DpiReport, String> {
    Ok(serde_json::from_value(json!({
        "requested":null,"stages":null,"active":{"x":1800,"y":1900},
        "acknowledgements":[],"verified":verified
    }))
    .unwrap())
}

#[test]
fn failed_ble_getter_retains_catalog_for_a_later_verified_cached_read() {
    let baseline = catalog_baseline();
    let scope = catalog_scope();
    let mut pending = None;
    let failure = Err("active DPI getter failed after OBM initialization".into());
    assert!(
        catalog_for_verified_read(
            &mut pending,
            Some(incoming_catalog(&baseline)),
            &baseline,
            scope,
            true,
            &failure,
        )
        .is_none()
    );
    assert_eq!(pending.as_ref().unwrap().baseline.serial_number, "");

    let mut observed = baseline.clone();
    observed.serial_number = "actual-ble-serial".into();
    for _ in 0..2 {
        // Cached reads have no new catalog or serial response. A failed window
        // delivery also leaves the retained input available to a later read.
        let projection = catalog_for_verified_read(
            &mut pending,
            None,
            &observed,
            scope,
            true,
            &read_result(true),
        )
        .unwrap();
        assert_eq!(projection.serial, "actual-ble-serial");
        assert_eq!(projection.catalog, "coherent current-serial catalog");
        assert_eq!(projection.baseline.serial_number, "");
        assert_eq!(
            projection.baseline.source_device_settings,
            Some(json!({"version":7}))
        );
    }
    assert!(pending.is_some());
}

#[test]
fn partial_read_and_cleanup_failure_keep_catalog_without_projecting_it() {
    let baseline = catalog_baseline();
    let scope = catalog_scope();
    let mut pending = None;
    assert!(
        catalog_for_verified_read(
            &mut pending,
            Some(incoming_catalog(&baseline)),
            &baseline,
            scope,
            true,
            &read_result(false),
        )
        .is_none()
    );
    assert!(pending.is_some());
    assert!(
        catalog_for_verified_read(
            &mut pending,
            None,
            &baseline,
            scope,
            true,
            &Err("service shutdown or join failed".into()),
        )
        .is_none()
    );
    assert!(pending.is_some());
}

#[test]
fn verified_write_does_not_import_a_retained_ble_catalog() {
    let baseline = catalog_baseline();
    let mut pending = None;
    assert!(
        catalog_for_verified_read(
            &mut pending,
            Some(incoming_catalog(&baseline)),
            &baseline,
            catalog_scope(),
            false,
            &read_result(true),
        )
        .is_none()
    );
    assert!(pending.is_some());
    assert!(
        catalog_for_verified_read(
            &mut pending,
            None,
            &baseline,
            catalog_scope(),
            true,
            &read_result(true),
        )
        .is_some()
    );
}

#[test]
fn local_profile_edits_or_selection_discard_the_original_catalog_baseline() {
    let baseline = catalog_baseline();
    let mut edited = baseline.clone();
    edited.profiles[0].name = "User renamed profile".into();
    let mut selected = baseline.clone();
    selected.active_profile = "local-second".into();
    let mut source_edited = baseline.clone();
    source_edited.source_device_settings = Some(json!({"version":8}));
    let mut replaced = baseline.clone();
    replaced.device_container_id = "replacement-container".into();
    let mut other_serial = baseline.clone();
    other_serial.serial_number = "another-actual-serial".into();
    for current in [edited, selected, source_edited, replaced, other_serial] {
        let mut pending = Some(incoming_catalog(&baseline));
        assert!(
            catalog_for_verified_read(
                &mut pending,
                None,
                &current,
                catalog_scope(),
                true,
                &read_result(true),
            )
            .is_none()
        );
        assert!(pending.is_none());
    }
}

#[test]
fn profile_scope_switch_discards_catalog_even_after_selection_returns_to_baseline() {
    let baseline = catalog_baseline();
    for scope in [
        MousePollingScope::new(super::EntityId::from(1), 4, 3),
        MousePollingScope::new(super::EntityId::from(2), 2, 3),
        MousePollingScope::new(super::EntityId::from(1), 2, 4),
    ] {
        let mut pending = Some(incoming_catalog(&baseline));
        discard_stale_ble_catalog(&mut pending, &baseline, scope);
        assert!(pending.is_none());
        assert!(
            catalog_for_verified_read(
                &mut pending,
                Some(incoming_catalog(&baseline)),
                &baseline,
                scope,
                true,
                &read_result(true),
            )
            .is_none()
        );
        assert!(pending.is_none());
    }
}

#[test]
fn ble_catalog_receipts_require_the_current_generation_and_device_connection() {
    let scope = catalog_scope();
    assert!(current_ble_completion(true, 11, scope, Some(11), scope));
    for (current, generation, request_scope) in [
        (false, Some(11), scope),
        (true, Some(10), scope),
        (true, None, scope),
        (
            true,
            Some(11),
            MousePollingScope::new(super::EntityId::from(2), 2, 3),
        ),
        (
            true,
            Some(11),
            MousePollingScope::new(super::EntityId::from(1), 2, 4),
        ),
    ] {
        assert!(!current_ble_completion(
            current,
            11,
            scope,
            generation,
            request_scope,
        ));
    }
}

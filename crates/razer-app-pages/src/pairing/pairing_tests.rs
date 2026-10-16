//! In-memory state and native event tests. No DLL, worker or hardware transport.
use super::{
    Lane, PairingDevice, PairingPage, PairingRequest, PairingState, PairingStatus,
    product_pairing_target,
};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, px, size};
use serde_json::{Value, json};

fn device(id: u32, category: &str, connected: u32) -> Value {
    json!({"productId": id, "productName": format!("Test {category} {id}"),
        "serialNumber": format!("fixture-{id}"), "category": category, "connected": connected})
}

fn state(dual: bool) -> PairingState {
    let mut state = PairingState::default();
    let mut master = device(1000, "MOUSE", 1);
    master["canPairTwoDevices"] = json!(dual);
    state.initialize_device_info(master).unwrap();
    read(&mut state, json!([]));
    state
}

fn read(state: &mut PairingState, values: Value) {
    let ticket = state.begin(PairingRequest::ReadBindings).unwrap();
    assert!(matches!(
        state.receive(&ticket, Ok(values)),
        Some(PairingRequest::Scan(Lane::Primary))
    ));
}

fn scan(state: &mut PairingState, lane: Lane, values: Value) -> Option<PairingRequest> {
    let ticket = state.begin(PairingRequest::Scan(lane)).unwrap();
    state.receive(&ticket, Ok(values))
}

#[test]
fn protocol_statuses_keep_card_unbinding_and_confirmed_bind_distinct() {
    let statuses = [
        PairingStatus::Initializing,
        PairingStatus::Ready,
        PairingStatus::Scanning,
        PairingStatus::ScanResults,
        PairingStatus::Binding,
        PairingStatus::Bound,
        PairingStatus::BindError,
        PairingStatus::CardUnbinding,
        PairingStatus::Unbinding,
        PairingStatus::Unbound,
        PairingStatus::UnbindError,
        PairingStatus::BindConfirmed,
    ];
    assert_eq!(
        statuses.map(|status| status as u8),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
}

#[test]
fn product_pairing_opener_preserves_audited_display_mode_metadata() {
    let mut raw = device(182, "MOUSE", 1);
    raw["deviceContainerId"] = json!("container-182");
    raw["canPairTwoDevices"] = json!(true);
    raw["isProductivity"] = json!(false);
    raw["deviceName"] = json!("DeathAdder V3 Pro");
    raw["allMasters"] = json!([{"productId": 9000, "dongleId": 700}]);
    raw["lang"] = json!("zh-CN");
    raw["deviceMetadata"] = json!({"revision": 3, "capabilities": ["pairing"]});
    let target = product_pairing_target(&raw).unwrap();
    assert_eq!(target.product_id(), 182);
    assert_eq!(target.container_id(), "container-182");
    assert_eq!(
        target.device_info(),
        &json!({
            "productId": 182,
            "deviceContainerId": "container-182",
            "serialNumber": "fixture-182",
            "productName": "Test MOUSE 182",
            "connected": 1,
            "category": "MOUSE",
            "canPairTwoDevices": true,
            "isProductivity": false,
            "deviceName": "DeathAdder V3 Pro",
            "allMasters": [{"productId": 9000, "dongleId": 700}],
            "lang": "zh-CN",
            "deviceMetadata": {"revision": 3, "capabilities": ["pairing"]},
        })
    );
}

#[test]
fn product_pairing_guard_accepts_container_alias_without_rewriting_owner_payload() {
    let raw = json!({
        "productId": 182,
        "containerId": "alias-container",
        "allMasters": "[{\"productId\":9000}]",
        "serialNumber": null,
    });
    let target = product_pairing_target(&raw).unwrap();
    assert_eq!(target.product_id(), 182);
    assert_eq!(target.container_id(), "alias-container");
    assert_eq!(target.device_info(), &raw);
    assert!(target.device_info().get("deviceContainerId").is_none());

    let raw = json!({"productId": 182, "deviceContainerId": "primary", "containerId": "alias"});
    assert_eq!(
        product_pairing_target(&raw).unwrap().container_id(),
        "primary"
    );
}

#[test]
fn product_pairing_guard_rejects_invalid_product_or_container_identity() {
    for raw in [
        json!({"productId": 0, "deviceContainerId": "container"}),
        json!({"productId": u64::from(u32::MAX) + 1, "deviceContainerId": "container"}),
        json!({"productId": -1, "deviceContainerId": "container"}),
        json!({"productId": "182", "deviceContainerId": "container"}),
        json!({"deviceContainerId": "container"}),
        json!({"productId": 182}),
        json!({"productId": 182, "deviceContainerId": ""}),
        json!({"productId": 182, "containerId": " \t\n"}),
        json!({"productId": 182, "deviceContainerId": " ", "containerId": "alias"}),
        json!({"productId": 182, "deviceContainerId": null}),
    ] {
        assert!(product_pairing_target(&raw).is_none(), "{raw}");
    }
}

#[test]
fn dual_scan_retains_the_other_lane_and_auto_pairs_each_empty_lane_once() {
    let mut state = state(true);
    read(&mut state, json!([]));
    let keyboard = device(2000, "KEYBOARD", 1);
    let next = scan(&mut state, Lane::Primary, json!([keyboard.clone()])).unwrap();
    assert!(matches!(next, PairingRequest::Bind(_)));
    let bind = state.begin(next).unwrap();
    assert_eq!(state.status(Lane::Primary), PairingStatus::Binding);
    assert!(matches!(
        state.receive(&bind, Ok(json!({"device": keyboard}))),
        Some(PairingRequest::Scan(Lane::Secondary))
    ));
    let mouse = device(3000, "MOUSE", 1);
    let next = scan(&mut state, Lane::Secondary, json!([mouse.clone()])).unwrap();
    let bind = state.begin(next).unwrap();
    state.receive(&bind, Ok(json!({"device": mouse})));
    assert_eq!(state.status(Lane::Primary), PairingStatus::BindConfirmed);
    assert_eq!(state.status(Lane::Secondary), PairingStatus::BindConfirmed);
    scan(
        &mut state,
        Lane::Primary,
        json!([device(2100, "KEYBOARD", 1)]),
    );
    scan(
        &mut state,
        Lane::Secondary,
        json!([device(3100, "MOUSE", 1)]),
    );
    let products: Vec<_> = state
        .cards()
        .iter()
        .map(|card| card.device().product_id())
        .collect();
    assert_eq!(products, [2000, 3000, 2100, 3100]);
}

#[test]
fn malformed_scan_is_an_error_while_empty_array_is_a_valid_result() {
    let mut state = state(false);
    let ticket = state.begin(PairingRequest::Scan(Lane::Primary)).unwrap();
    state.receive(&ticket, Ok(json!({"devices": []})));
    assert!(state.error().is_some());
    assert_eq!(state.status(Lane::Primary), PairingStatus::ScanResults);
    assert!(state.cards().is_empty());
    scan(&mut state, Lane::Primary, json!([]));
    assert!(state.error().is_none());
    assert!(state.cards().is_empty());
    assert!(state.begin(PairingRequest::Scan(Lane::Secondary)).is_err());
}

#[test]
fn binding_uses_observed_payload_and_does_not_invent_the_selected_record() {
    let mut state = state(false);
    scan(
        &mut state,
        Lane::Primary,
        json!([device(2000, "KEYBOARD", 1)]),
    );
    let next = state
        .pair_request(state.cards()[0].device().clone(), false)
        .unwrap();
    let ticket = state.begin(next).unwrap();
    let observed = json!({"productId": 2100, "category": "KEYBOARD", "connected": 1});
    state.receive(&ticket, Ok(json!({"device": observed.clone()})));
    assert_eq!(state.status(Lane::Primary), PairingStatus::BindConfirmed);
    assert_eq!(state.cards()[0].device().product_id(), 2100);
    assert_eq!(state.cards()[0].device().name("en"), "");

    let mut state = super::state::PairingState::default();
    state
        .initialize_device_info(json!({"productId":1000,"category":"MOUSE"}))
        .unwrap();
    scan(
        &mut state,
        Lane::Primary,
        json!([device(2000, "KEYBOARD", 1)]),
    );
    let next = state
        .pair_request(state.cards()[0].device().clone(), false)
        .unwrap();
    let ticket = state.begin(next).unwrap();
    state.receive(&ticket, Ok(json!({})));
    assert_eq!(state.status(Lane::Primary), PairingStatus::Binding);
    assert!(state.cards().iter().all(|card| !card.paired()));
}

#[test]
fn card_projection_deduplicates_products_and_only_offers_complementary_devices() {
    let mut state = state(false);
    let target = device(2000, "KEYBOARD", 1);
    let mut duplicate = target.clone();
    duplicate["serialNumber"] = json!("another-unit");
    read(&mut state, json!([target.clone(), duplicate.clone()]));
    state
        .set_external(json!([duplicate, device(3000, "MOUSE", 1)]))
        .unwrap();
    scan(
        &mut state,
        Lane::Primary,
        json!([
            target,
            device(1000, "MOUSE", 1),
            device(2100, "KEYBOARD", 1)
        ]),
    );
    let cards = state.cards();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].device().product_id(), 2000);
    assert_eq!(cards[1].device().product_id(), 2100);
    assert!(!state.cards()[1].paired());
}

#[test]
fn unbinding_uses_the_source_optimistic_ui_record_and_keeps_it_after_failure() {
    let mut state = state(false);
    let target = device(2000, "KEYBOARD", 1);
    read(&mut state, json!([target]));
    let device = state.cards()[0].device().clone();
    let request = PairingRequest::Unbind {
        device: device.clone(),
        external: false,
    };
    let ticket = state.begin(request.clone()).unwrap();
    assert_eq!(state.status(Lane::Primary), PairingStatus::Unbinding);
    assert_eq!(state.cards()[0].status(), PairingStatus::CardUnbinding);
    assert!(state.cards()[0].device().disconnected());
    state.receive(&ticket, Err("source unbind failure".into()));
    assert_eq!(state.status(Lane::Primary), PairingStatus::UnbindError);
    state.recover(
        state.generation(),
        Lane::Primary,
        PairingStatus::UnbindError,
    );
    assert!(!state.cards()[0].paired());
    let target = state.cards()[0].device().clone();
    let request = PairingRequest::Unbind {
        device: target,
        external: false,
    };
    let ticket = state.begin(request).unwrap();
    state.receive(&ticket, Ok(json!({"productId": "2000"})));
    assert_eq!(state.status(Lane::Primary), PairingStatus::Unbound);
    assert!(state.cards()[0].device().disconnected());
}

#[test]
fn replacing_a_binding_unpairs_first_and_only_binds_after_acknowledgement() {
    let mut state = state(false);
    read(&mut state, json!([device(2000, "KEYBOARD", 1)]));
    scan(
        &mut state,
        Lane::Primary,
        json!([device(2100, "KEYBOARD", 1)]),
    );
    let target = state.cards()[1].device().clone();
    let request = state.pair_request(target.clone(), false).unwrap();
    assert_eq!(
        request.payload(false),
        json!({"productId":2000,"category":null})
    );
    let ticket = state.begin(request).unwrap();
    let next = state
        .receive(&ticket, Ok(json!({"productId":2000})))
        .unwrap();
    assert_eq!(next.payload(false)["device"]["productId"], 2100);
    let ticket = state.begin(next).unwrap();
    state.receive(&ticket, Ok(json!({"device":device(2100,"KEYBOARD",1)})));
    assert_eq!(state.cards().len(), 1);
    assert_eq!(state.cards()[0].device().key(), target.key());
    assert!(state.cards()[0].paired());
}

#[test]
fn old_request_and_old_receiver_responses_cannot_change_current_state() {
    let mut state = state(false);
    let old = state.begin(PairingRequest::Scan(Lane::Primary)).unwrap();
    state.invalidate();
    let current = state.begin(PairingRequest::Scan(Lane::Primary)).unwrap();
    assert!(
        state
            .receive(&old, Ok(json!([device(2000, "KEYBOARD", 1)])))
            .is_none()
    );
    assert!(state.current(&current));
    assert!(state.cards().is_empty());
    state.replace_masters(json!([])).unwrap();
    state.receive(&current, Ok(json!([device(2100, "KEYBOARD", 1)])));
    assert!(state.master().is_none());
    assert!(state.cards().is_empty());
    assert!(!state.busy());
}

#[test]
fn external_unpair_routes_to_recorded_owner_and_fallback_never_fakes_success() {
    let mut state = state(false);
    let mut target = device(2000, "KEYBOARD", 2);
    target["dongleId"] = json!(700);
    target["master"] = json!({"productId": 9000});
    target["deviceContainerId"] = json!("owner-container");
    state.set_external(json!([target])).unwrap();
    let target = state.cards()[0].device().clone();
    let request = PairingRequest::Unbind {
        device: target.clone(),
        external: true,
    };
    assert_eq!(
        request.routing_context(state.context().unwrap()).unwrap(),
        json!({"productId":9000,"deviceContainerId":"owner-container"})
    );
    assert_eq!(
        request.payload(false),
        json!({"productId":700,"category":null})
    );
    let first = state.begin(request).unwrap();
    let next = state.external_fallback(&first).unwrap();
    assert_eq!(state.status(Lane::Primary), PairingStatus::Ready);
    assert!(!state.cards()[0].device().disconnected());
    assert_eq!(next.payload(false)["productId"], 2000);
    let second = state.begin(next).unwrap();
    state.receive(&first, Ok(json!({"productId":700})));
    assert!(state.current(&second));
    assert!(
        state
            .receive(&second, Ok(json!({"productId":2000})))
            .is_none()
    );
    assert!(state.cards()[0].device().disconnected());
    assert!(!state.cards()[0].actionable());
}

#[test]
fn external_reclaim_scans_on_timeout_without_claiming_unbind_success() {
    let mut state = state(false);
    let mut target = device(2000, "KEYBOARD", 2);
    target["dongleId"] = json!(700);
    target["master"] = json!({"productId":9000});
    state.set_external(json!([target])).unwrap();
    let target = state.cards()[0].device().clone();
    let request = state.pair_request(target, true).unwrap();
    assert!(matches!(request, PairingRequest::ReclaimExternal(_)));
    let reclaim = state.begin(request).unwrap();
    assert!(matches!(
        state.external_fallback(&reclaim),
        Some(PairingRequest::Scan(Lane::Primary))
    ));
    assert!(!state.cards()[0].device().disconnected());
    let next = scan(
        &mut state,
        Lane::Primary,
        json!([device(2000, "KEYBOARD", 1)]),
    )
    .unwrap();
    assert!(matches!(next, PairingRequest::Bind(_)));
}

#[test]
fn reclaim_uses_the_current_source_product_id_lookup() {
    let mut state = state(false);
    let mut target = device(2000, "KEYBOARD", 2);
    target["master"] = json!({"productId":9000});
    state.set_external(json!([target])).unwrap();
    let request = state
        .pair_request(state.cards()[0].device().clone(), true)
        .unwrap();
    let ticket = state.begin(request).unwrap();
    state.receive(&ticket, Ok(json!({"productId":2000})));
    let mut observed = device(2000, "KEYBOARD", 1);
    observed["serialNumber"] = json!("current-observed-unit");
    let request = scan(&mut state, Lane::Primary, json!([observed])).unwrap();
    assert_eq!(
        request.payload(false)["device"]["serialNumber"],
        "current-observed-unit"
    );
}

#[test]
fn failed_keyboard_scan_continues_mouse_scan_and_bind_recovery_clears_both_lanes() {
    let mut state = state(true);
    let ticket = state.begin(PairingRequest::Scan(Lane::Primary)).unwrap();
    assert!(matches!(
        state.receive(&ticket, Err("scan failed".into())),
        Some(PairingRequest::Scan(Lane::Secondary))
    ));
    let keyboard = device(2000, "KEYBOARD", 1);
    let next = scan(&mut state, Lane::Primary, json!([keyboard])).unwrap();
    let ticket = state.begin(next).unwrap();
    let next = state.receive(&ticket, Err("bind failed".into())).unwrap();
    assert!(matches!(next, PairingRequest::Scan(Lane::Secondary)));
    let mouse = device(3000, "MOUSE", 1);
    let ticket = state.begin(next).unwrap();
    let next = state.receive(&ticket, Ok(json!([mouse.clone()]))).unwrap();
    let ticket = state.begin(next).unwrap();
    state.receive(&ticket, Ok(json!({"device": mouse})));
    assert!(state.cards()[0].paired());
    assert_eq!(state.cards()[0].device().product_id(), 3000);
    state.recover(state.generation(), Lane::Primary, PairingStatus::BindError);
    assert_eq!(state.status(Lane::Primary), PairingStatus::Ready);
    assert_eq!(state.status(Lane::Secondary), PairingStatus::Ready);
    assert!(state.cards().is_empty());
}

#[test]
fn ordinary_unbind_accepts_missing_or_different_response_ids_and_categories() {
    for response in [
        json!({}),
        json!({"productId":9999,"category":"MOUSE"}),
        json!({"productId":2000,"category":"MOUSE"}),
    ] {
        let mut state = state(false);
        read(&mut state, json!([device(2000, "KEYBOARD", 1)]));
        let request = PairingRequest::Unbind {
            device: state.cards()[0].device().clone(),
            external: false,
        };
        let ticket = state.begin(request).unwrap();
        state.receive(&ticket, Ok(response));
        assert_eq!(state.status(Lane::Primary), PairingStatus::Unbound);
        assert!(state.error().is_none());
        assert!(state.cards()[0].device().disconnected());
    }
}

#[test]
fn ordinary_unbind_marks_the_response_product_in_addition_to_the_optimistic_target() {
    let mut state = state(false);
    read(
        &mut state,
        json!([device(2000, "KEYBOARD", 1), device(2100, "KEYBOARD", 1),]),
    );
    let request = PairingRequest::Unbind {
        device: state.cards()[0].device().clone(),
        external: false,
    };
    let ticket = state.begin(request).unwrap();
    state.receive(&ticket, Ok(json!({"productId":2100})));
    assert!(
        state
            .cards()
            .iter()
            .all(|card| card.device().disconnected())
    );
    assert!(state.error().is_none());
}

#[test]
fn external_reclaim_uses_successful_owner_response_without_payload_id_filtering() {
    let mut state = state(false);
    state
        .set_external(json!([device(2000, "KEYBOARD", 2)]))
        .unwrap();
    let request = state
        .pair_request(state.cards()[0].device().clone(), true)
        .unwrap();
    let ticket = state.begin(request).unwrap();
    assert!(matches!(
        state.receive(&ticket, Ok(json!({}))),
        Some(PairingRequest::Scan(Lane::Primary))
    ));
    assert!(state.error().is_none());
    assert!(!state.cards()[0].device().disconnected());
}

#[test]
fn external_record_changes_invalidate_requests_but_preserve_local_bindings() {
    let mut state = state(false);
    read(&mut state, json!([device(2100, "KEYBOARD", 1)]));
    let mut target = device(2000, "KEYBOARD", 2);
    target["master"] = json!({"productId":9000});
    state.set_external(json!([target])).unwrap();
    let target = state.cards()[1].device().clone();
    let request = state.pair_request(target, true).unwrap();
    let ticket = state.begin(request).unwrap();
    state.set_external(json!([])).unwrap();
    assert!(!state.current(&ticket));
    assert!(!state.busy());
    assert!(state.cards()[0].paired());
    assert!(
        state
            .receive(&ticket, Ok(json!({"productId":2000})))
            .is_none()
    );
}

#[test]
fn dual_category_requests_keep_both_tickets_until_their_own_responses() {
    let mut state = state(true);
    let keyboard = device(2000, "KEYBOARD", 1);
    let mouse = device(3000, "MOUSE", 1);
    scan(&mut state, Lane::Primary, json!([keyboard.clone()]));
    scan(&mut state, Lane::Secondary, json!([mouse.clone()]));
    let keyboard = PairingDevice::parse(keyboard).unwrap();
    let mouse = PairingDevice::parse(mouse).unwrap();
    let first = state.begin(PairingRequest::Bind(keyboard.clone())).unwrap();
    let second = state.begin(PairingRequest::Bind(mouse.clone())).unwrap();
    assert!(state.current(&first));
    assert!(state.current(&second));
    state.receive(&second, Ok(json!({"device":device(3000, "MOUSE", 1)})));
    assert!(state.current(&first));
    assert!(!state.current(&second));
    assert_eq!(state.status(Lane::Primary), PairingStatus::Binding);
    assert_eq!(state.status(Lane::Secondary), PairingStatus::BindConfirmed);
    state.receive(&first, Ok(json!({"device":device(2000, "KEYBOARD", 1)})));
    assert!(!state.busy());
    assert!(state.cards().iter().all(|card| card.paired()));
}

#[test]
fn receiver_reinitialization_invalidates_both_pending_categories() {
    let mut state = state(true);
    let first = state.begin(PairingRequest::Scan(Lane::Primary)).unwrap();
    let second = state.begin(PairingRequest::Scan(Lane::Secondary)).unwrap();
    state
        .initialize_device_info(json!({"productId":207,"canPairTwoDevices":true}))
        .unwrap();
    for ticket in [first, second] {
        assert!(!state.current(&ticket));
        state.receive(&ticket, Ok(json!([device(2000, "KEYBOARD", 1)])));
    }
    assert!(state.cards().is_empty());
    assert_eq!(state.status(Lane::Primary), PairingStatus::Initializing);
}

#[test]
fn unmatched_external_acknowledgement_keeps_its_marker_and_original_lane_status() {
    let mut state = state(false);
    let target = device(2000, "KEYBOARD", 2);
    state.set_external(json!([target])).unwrap();
    let request = PairingRequest::Unbind {
        device: state.cards()[0].device().clone(),
        external: true,
    };
    let ticket = state.begin(request).unwrap();
    state.receive(&ticket, Ok(json!({"productId":9999})));
    assert!(state.current(&ticket));
    assert!(state.error().is_none());
    assert_eq!(state.status(Lane::Primary), PairingStatus::Ready);
    assert_eq!(state.cards()[0].status(), PairingStatus::CardUnbinding);
    state.expire_external(&ticket);
    assert!(!state.current(&ticket));
    assert_eq!(state.status(Lane::Primary), PairingStatus::Ready);
    assert!(!state.cards()[0].device().disconnected());
}

#[test]
fn reclaim_error_keeps_the_source_two_second_fallback_available() {
    let mut state = state(false);
    state
        .set_external(json!([device(2000, "KEYBOARD", 2)]))
        .unwrap();
    let request = state
        .pair_request(state.cards()[0].device().clone(), true)
        .unwrap();
    let ticket = state.begin(request).unwrap();
    state.receive(&ticket, Err("observed reclaim failure".into()));
    assert!(state.current(&ticket));
    assert_eq!(state.status(Lane::Primary), PairingStatus::Ready);
    assert_eq!(state.cards()[0].status(), PairingStatus::Unbound);
    assert!(matches!(
        state.external_fallback(&ticket),
        Some(PairingRequest::Scan(Lane::Primary))
    ));
}

#[test]
fn bind_info_treats_a_successful_non_array_as_the_source_empty_list() {
    let mut state = state(false);
    read(&mut state, json!({"devices":[]}));
    assert!(state.cards().is_empty());
    assert!(state.error().is_none());
    assert_eq!(state.status(Lane::Primary), PairingStatus::Ready);
}

#[test]
fn simultaneous_scan_responses_follow_source_status_order_without_ticket_category_inference() {
    let mut state = state(true);
    read(
        &mut state,
        json!([device(2000, "KEYBOARD", 1), device(3000, "MOUSE", 1)]),
    );
    let keyboard = state.begin(PairingRequest::Scan(Lane::Primary)).unwrap();
    let mouse = state.begin(PairingRequest::Scan(Lane::Secondary)).unwrap();
    state.receive(&mouse, Ok(json!([device(3100, "MOUSE", 1)])));
    assert!(state.current(&keyboard));
    assert!(!state.current(&mouse));
    assert_eq!(state.status(Lane::Primary), PairingStatus::ScanResults);
    assert_eq!(state.status(Lane::Secondary), PairingStatus::ScanResults);
    assert!(
        state
            .cards()
            .iter()
            .any(|card| card.device().product_id() == 3100)
    );
    // Neither status is Scanning now: the original merger's null category
    // branch replaces the scanned list rather than inferring the ticket lane.
    state.receive(&keyboard, Ok(json!([device(2100, "KEYBOARD", 1)])));
    assert!(
        !state
            .cards()
            .iter()
            .any(|card| card.device().product_id() == 3100)
    );
    assert!(
        state
            .cards()
            .iter()
            .any(|card| card.device().product_id() == 2100)
    );
}

#[test]
fn simultaneous_binding_error_updates_the_first_binding_status_as_in_source() {
    let mut state = state(true);
    let keyboard = device(2000, "KEYBOARD", 1);
    let mouse = device(3000, "MOUSE", 1);
    scan(&mut state, Lane::Primary, json!([keyboard.clone()]));
    scan(&mut state, Lane::Secondary, json!([mouse.clone()]));
    let keyboard = state
        .begin(PairingRequest::Bind(
            PairingDevice::parse(keyboard).unwrap(),
        ))
        .unwrap();
    let mouse = state
        .begin(PairingRequest::Bind(PairingDevice::parse(mouse).unwrap()))
        .unwrap();
    state.receive(&mouse, Err("observed shared broadcast error".into()));
    assert!(state.current(&keyboard));
    assert_eq!(state.status(Lane::Primary), PairingStatus::BindError);
    assert_eq!(state.status(Lane::Secondary), PairingStatus::Binding);
    assert_eq!(
        state.recovery_request(),
        Some((Lane::Primary, PairingStatus::BindError))
    );
}

#[test]
fn runtime_sleep_metadata_dims_a_binding_without_unpairing_it() {
    for runtime in [
        json!({"devicePowerState":"Sleep"}),
        json!({"powerStatus":{"chargingStatus":"OFF"}}),
        json!({"chargingStatus":"off"}),
    ] {
        let mut target = device(2000, "KEYBOARD", 2);
        target
            .as_object_mut()
            .unwrap()
            .extend(runtime.as_object().unwrap().clone());
        let target = PairingDevice::parse(target).unwrap();
        assert!(target.dimmed());
        assert!(!target.connected());
        assert!(!target.disconnected());
    }
    let mut target = device(2000, "KEYBOARD", 2);
    target["productName"] = json!({"zh-cn":"键盘", "en":"Keyboard"});
    assert_eq!(PairingDevice::parse(target).unwrap().name("zh-cn"), "键盘");
}

#[test]
fn invalid_master_is_rejected_and_external_owner_falls_back_to_current_source_owner() {
    let mut state = state(false);
    let key = state.master().unwrap().key().to_string();
    assert!(
        state
            .replace_masters(json!([{"productId":1,"productName":"No serial"}]))
            .is_err()
    );
    assert_eq!(state.master().unwrap().key(), key);
    let request = PairingRequest::Unbind {
        device: PairingDevice::parse(device(2000, "KEYBOARD", 2)).unwrap(),
        external: true,
    };
    assert_eq!(
        request.routing_context(state.context().unwrap()).unwrap(),
        state.context().unwrap()
    );
}

fn open_page(
    cx: &mut TestAppContext,
    fixture: Option<PairingState>,
) -> (WindowHandle<Root>, Entity<PairingPage>) {
    let mut page = None;
    let handle = cx.open_window(size(px(1100.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            let mut view = PairingPage::new(window, cx);
            if let Some(fixture) = fixture {
                view.state = fixture;
            }
            view
        });
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    (handle, page.unwrap())
}

#[gpui_kit::test]
fn initial_pairing_loading_has_no_unmounted_toolbar_or_status_messages(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, page) = open_page(cx, None);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("pairing-initial-loading").is_some());
        for id in [
            "pairing-scan-primary",
            "pairing-back",
            "pairing-refresh",
            "pairing-error",
            "multi-device-pairing",
        ] {
            assert!(window.try_find(id).is_none());
        }
    })
    .unwrap();
    cx.update(|cx| {
        let page = page.read(cx);
        assert_eq!(
            page.state.status(Lane::Primary),
            PairingStatus::Initializing
        );
        assert!(page.state.cards().is_empty());
        assert!(!page.state.busy());
    });
}

#[gpui_kit::test]
fn confirmation_escape_closes_without_invented_focus_restore_and_enter_cannot_fake_binding(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let mut fixture = state(false);
    scan(
        &mut fixture,
        Lane::Primary,
        json!([device(2000, "KEYBOARD", 1)]),
    );
    let (handle, page) = open_page(cx, Some(fixture));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("pairing-action-2000:fixture-2000", cx);
        assert!(window.try_find("pairing-confirmation").is_some());
        window.press("escape", cx);
        assert!(window.try_find("pairing-confirmation").is_none());
        assert!(window.focused(cx).is_none());
        window.click("pairing-action-2000:fixture-2000", cx);
        window.press("enter", cx);
        assert!(window.try_find("pairing-confirmation").is_none());
    })
    .unwrap();
    cx.update(|cx| {
        let page = page.read(cx);
        assert!(!page.state.busy());
        assert!(page.state.cards().iter().all(|card| !card.paired()));
        assert!(page.state.error().is_some());
    });
}

#[gpui_kit::test]
fn inline_confirmation_uses_content_height_and_a_centered_compact_button(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut fixture = state(false);
    scan(
        &mut fixture,
        Lane::Primary,
        json!([device(2000, "KEYBOARD", 1)]),
    );
    let (handle, _) = open_page(cx, Some(fixture));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("pairing-action-2000:fixture-2000", cx);
        let backdrop = window.find("pairing-confirmation").bounds();
        let dialog = window.find("pairing-confirm-dialog").bounds();
        let button = window.find("pairing-confirm-continue").bounds();
        let scale = window.rem_size() / 16.;
        assert_eq!(backdrop.size.width, scale * 290.);
        assert_eq!(dialog.size.width, scale * 230.);
        assert!(dialog.size.height < scale * 220.);
        assert_eq!(dialog.origin.y, backdrop.origin.y);
        assert_eq!(button.size.width, scale * 90.);
        assert_eq!(button.size.height, scale * 27.);
        assert!(
            (f32::from(button.origin.x + button.size.width / 2.)
                - f32::from(dialog.origin.x + dialog.size.width / 2.))
            .abs()
                <= 1.
        );
    })
    .unwrap();
}

#[test]
fn source_713_continuation_uses_the_first_matching_scan_entry() {
    let mut state = state(false);
    let mut first = device(2000, "KEYBOARD", 1);
    first["dongleId"] = json!(713);
    let mut second = device(2100, "KEYBOARD", 1);
    second["dongleId"] = json!(713);
    assert!(scan(&mut state, Lane::Primary, json!([first, second])).is_none());
    let selected = state.cards()[1].device().clone();
    let request = state.pair_request(selected, false).unwrap();
    state.begin(request).unwrap();
    assert_eq!(state.status(Lane::Primary), PairingStatus::Binding);
    let continued = state.continue_713().unwrap();
    assert_eq!(continued.payload(false)["device"]["productId"], 2000);
    assert!(!state.busy());
}

#[test]
fn real_source_owner_initialization_does_not_require_a_fabricated_device_name_or_serial() {
    let mut state = PairingState::default();
    state
        .initialize_device_info(json!({"productId":207,"canPairTwoDevices":true}))
        .unwrap();
    assert!(state.dual());
    assert_eq!(
        state.context().unwrap(),
        json!({"productId":207,"canPairTwoDevices":true})
    );
    assert_eq!(state.status(Lane::Primary), PairingStatus::Initializing);
    assert!(state.cards().is_empty());
}

//! In-memory state and native event tests. No DLL, worker or hardware transport.
use super::{Lane, PairingDevice, PairingPage, PairingRequest, PairingState, PairingStatus};
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
    state.replace_masters(json!([master])).unwrap();
    let key = state.masters()[0].key().to_string();
    state.select_master(&key).unwrap();
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
fn binding_requires_device_payload_matching_product_category_and_known_serial() {
    let target = device(2000, "KEYBOARD", 1);
    for response in [
        json!({}),
        json!({"device": device(2001, "KEYBOARD", 1)}),
        json!({"device": device(2000, "MOUSE", 1)}),
        json!({"device": {"productId":2000,"productName":"Other unit","category":"KEYBOARD","serialNumber":"other"}}),
    ] {
        let mut state = state(false);
        let next = scan(&mut state, Lane::Primary, json!([target.clone()])).unwrap();
        let ticket = state.begin(next).unwrap();
        state.receive(&ticket, Ok(response));
        assert_eq!(state.status(Lane::Primary), PairingStatus::BindError);
        assert!(state.cards().iter().all(|card| !card.paired()));
        state.recover(state.generation(), Lane::Primary, PairingStatus::BindError);
        assert_eq!(state.status(Lane::Primary), PairingStatus::Ready);
        assert!(state.cards().is_empty());
        // The source's automatic first-candidate action only runs once per page/receiver.
        assert!(scan(&mut state, Lane::Primary, json!([target.clone()])).is_none());
    }
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
    state.select(cards[1].device().key());
    assert!(!state.cards()[1].paired());
}

#[test]
fn unbinding_waits_for_matching_ack_and_failure_recovers_the_real_binding() {
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
    assert!(!state.cards()[0].device().disconnected());
    state.receive(&ticket, Ok(json!({"productId": 9999})));
    assert_eq!(state.status(Lane::Primary), PairingStatus::UnbindError);
    state.recover(
        state.generation(),
        Lane::Primary,
        PairingStatus::UnbindError,
    );
    assert!(state.cards()[0].paired());
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
    let request = state.pair_request(target.clone(), false);
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
    let request = state.pair_request(target.clone(), true);
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
    assert_eq!(state.status(Lane::Primary), PairingStatus::Unbinding);
    assert!(!state.cards()[0].device().disconnected());
    assert_eq!(next.payload(false)["productId"], 2000);
    let second = state.begin(next).unwrap();
    state.receive(&first, Ok(json!({"productId":700})));
    assert!(state.current(&second));
    assert!(matches!(
        state.receive(&second, Ok(json!({"productId":2000}))),
        Some(PairingRequest::Scan(Lane::Primary))
    ));
    assert!(state.cards()[0].device().disconnected());
    let next = scan(
        &mut state,
        Lane::Primary,
        json!([device(2000, "KEYBOARD", 1)]),
    )
    .unwrap();
    assert!(matches!(next, PairingRequest::Bind(_)));
}

#[test]
fn invalid_master_or_missing_external_owner_is_rejected_without_losing_valid_metadata() {
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
    assert!(request.routing_context(state.context().unwrap()).is_err());
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
fn unavailable_scan_shows_an_immediate_error_and_never_starts_loading(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, page) = open_page(cx, None);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("pairing-scan-primary", cx);
        assert!(window.try_find("pairing-loading").is_none());
        assert!(window.try_find("pairing-error").is_some());
    })
    .unwrap();
    cx.update(|cx| {
        let page = page.read(cx);
        assert!(page.state.error().unwrap().contains("无线配对服务尚未接通"));
        assert!(!page.state.busy());
        assert!(page.state.cards().is_empty());
        assert!(page.request_task.is_none());
        assert!(page.watchdog.is_none());
    });
}

#[gpui_kit::test]
fn confirmation_escape_restores_focus_and_enter_cannot_fake_a_binding(cx: &mut TestAppContext) {
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
        assert_eq!(
            window.find("pairing-action-2000:fixture-2000").focused(),
            Some(true)
        );
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

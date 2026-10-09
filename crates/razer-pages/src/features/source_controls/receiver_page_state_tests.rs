//! Source contract regressions, compiled only under the audit policy.
use super::super::pairing_state::{PairingState, ReceiverOperation};
use super::*;

fn peer(edition: Option<u32>, category: &str) -> ReceiverPeer {
    serde_json::from_value(serde_json::json!({
        "productId": 183, "editionId": edition, "category": category,
        "productName": { "en": "RAZER VIPER V2 PRO" }
    }))
    .unwrap()
}

fn queried(peer: ReceiverPeer, devices: ReceiverDevicesObservation) -> ReceiverPageState {
    let mut state = ReceiverPageState {
        active: true,
        ..Default::default()
    };
    state.observe_devices(devices);
    let event = state.begin().unwrap();
    assert!(state.observe(&ReceiverPairingObservation::bindings(
        event.session(),
        vec![peer]
    )));
    state
}

#[test]
fn partial_discovery_preserves_positive_connection_and_navigation() {
    let mut state = queried(
        peer(Some(2), "MOUSE"),
        ReceiverDevicesObservation::complete(vec![(183, 2)]),
    );
    let partial = ReceiverDevicesObservation::partial(vec![(183, 2)]);
    assert!(!partial.is_complete());
    assert_eq!(partial.connected(), &[(183, 2)]);
    assert!(state.observe_devices(partial));
    assert_eq!(state.connected(), Some(true));
    assert!(!state.temporary());
    assert_eq!(state.navigation().unwrap().edition_id(), 2);

    // Another product was observed while this receiver/transport failed.
    assert!(state.observe_devices(ReceiverDevicesObservation::partial(vec![(653, 0)])));
    assert_eq!(state.connected(), None);
    assert!(state.navigation().is_none());
    assert!(!state.temporary());
    assert!(state.peer().is_some());
    assert!(state.observe_devices(ReceiverDevicesObservation::complete(vec![(653, 0)])));
    assert_eq!(state.connected(), Some(false));
}

#[test]
fn partial_new_binding_can_resolve_without_assuming_unknown_editions() {
    let state = queried(
        peer(Some(2), "MOUSE"),
        ReceiverDevicesObservation::partial(vec![(183, 2)]),
    );
    assert!(!state.temporary());
    assert!(state.navigation().is_some());
    let state = queried(
        peer(None, "MOUSE"),
        ReceiverDevicesObservation::partial(vec![(183, 2)]),
    );
    assert_eq!(state.connected(), Some(true));
    assert!(state.temporary());
    assert_eq!(state.peer().unwrap().edition_id(), None);
    assert_eq!(state.navigation().unwrap().edition_id(), 2);
    let state = queried(
        peer(Some(2), "MOUSE"),
        ReceiverDevicesObservation::partial(vec![(183, 1)]),
    );
    assert_eq!(state.connected(), Some(true));
    assert!(state.temporary());
    assert!(state.navigation().is_none());
}

#[test]
fn partial_snapshot_keeps_duplicate_owners_ambiguous_and_default_unknown() {
    let state = queried(
        peer(Some(2), "MOUSE"),
        ReceiverDevicesObservation::partial(vec![(183, 2), (183, 2)]),
    );
    assert_eq!(state.connected(), Some(true));
    assert!(state.navigation().is_none());
    let unknown = ReceiverDevicesObservation::default();
    assert!(!unknown.is_complete());
    assert!(unknown.connected().is_empty());
    let state = queried(peer(Some(2), "MOUSE"), unknown);
    assert_eq!(state.connected(), None);
    assert!(state.temporary());
    assert!(state.navigation().is_none());
}

#[test]
fn minimal_binding_never_invents_metadata_or_connection() {
    let state = queried(
        ReceiverPeer::queried(183, 1),
        ReceiverDevicesObservation::default(),
    );
    assert_eq!(state.peer().unwrap().label("en"), "PID 183");
    assert_eq!(state.peer().unwrap().edition_id(), None);
    assert_eq!(state.peer().unwrap().image_identity(), None);
    assert!(state.temporary());
    assert_eq!(state.connected(), None);
    assert!(state.navigation().is_none());
    assert!(!state.loading);
}

#[test]
fn confirmation_requires_exact_observed_edition_and_disconnect_keeps_binding() {
    let mut state = queried(
        peer(Some(2), "MOUSE"),
        ReceiverDevicesObservation::complete(vec![(183, 1)]),
    );
    assert!(state.temporary());
    assert_eq!(state.connected(), Some(true));
    assert!(state.navigation().is_none());
    assert!(state.loading);
    state.observe_devices(ReceiverDevicesObservation::complete(vec![(183, 2)]));
    assert!(!state.temporary());
    assert!(!state.loading);
    assert!(state.retry().is_none());
    let navigation = state.navigation().unwrap();
    assert_eq!((navigation.product_id(), navigation.edition_id()), (183, 2));
    assert_eq!(navigation.page(), crate::nav::Tab::Performance);
    state.observe_devices(ReceiverDevicesObservation::complete(vec![]));
    assert_eq!(state.connected(), Some(false));
    assert!(state.peer().is_some());
    assert!(!state.temporary());
    assert!(state.navigation().is_none());
    state.observe_devices(ReceiverDevicesObservation::default());
    assert_eq!(state.connected(), None);
}

#[test]
fn navigation_requires_unique_live_target_and_observed_category() {
    let mut state = queried(
        peer(None, "KEYBOARD"),
        ReceiverDevicesObservation::complete(vec![(183, 1), (183, 2)]),
    );
    assert!(state.navigation().is_none());
    state.observe_devices(ReceiverDevicesObservation::complete(vec![(183, 2)]));
    assert!(state.temporary());
    assert_eq!(state.peer().unwrap().edition_id(), None);
    assert_eq!(
        state.navigation().unwrap().page(),
        crate::nav::Tab::Customize
    );
    let state = queried(
        ReceiverPeer::queried(183, 1),
        ReceiverDevicesObservation::complete(vec![(183, 2)]),
    );
    assert!(state.navigation().is_none());
}

#[test]
fn temporary_connected_binding_has_thirty_delayed_retries_then_stops() {
    let mut state = ReceiverPageState {
        active: true,
        ..Default::default()
    };
    state.observe_devices(ReceiverDevicesObservation::complete(vec![(183, 1)]));
    let mut request = state.begin().unwrap();
    for attempt in 0..=30 {
        assert!(state.observe(&ReceiverPairingObservation::bindings(
            request.session(),
            vec![peer(None, "MOUSE")]
        )));
        if attempt < 30 {
            assert!(state.loading);
            request = state.resume_retry(state.retry().unwrap()).unwrap();
        } else {
            assert!(state.retry().is_none());
            assert!(!state.loading);
        }
    }
}

#[test]
fn leaving_and_dialog_generations_reject_stale_parent_results() {
    let mut state = ReceiverPageState::default();
    assert!(state.begin().is_none());
    state.active = true;
    let old = state.begin().unwrap();
    state.suspend();
    let mut dialog = PairingState::default();
    let dialog_query = dialog.open();
    assert_ne!(old.session(), dialog_query.session());
    assert!(!state.observe(&ReceiverPairingObservation::bindings(
        old.session(),
        vec![peer(Some(1), "MOUSE")]
    )));
    let current = state.begin().unwrap();
    assert_ne!(current.session(), dialog_query.session());
    assert!(!state.observe(&ReceiverPairingObservation::bindings(
        dialog_query.session(),
        vec![]
    )));
    assert!(state.observe(&ReceiverPairingObservation::failed(
        current.session(),
        ReceiverOperation::Bindings
    )));
    assert!(state.failed);
    assert!(!state.loading);
    let retry = state.begin().unwrap();
    assert!(state.observe(&ReceiverPairingObservation::bindings(
        retry.session(),
        vec![]
    )));
    assert!(!state.failed);
    assert!(state.peer().is_none());
}

#[test]
fn canceled_retry_cannot_resume_or_publish_binding() {
    let mut state = queried(
        peer(None, "MOUSE"),
        ReceiverDevicesObservation::complete(vec![(183, 1)]),
    );
    let ticket = state.retry().unwrap();
    state.active = false;
    state.suspend();
    assert!(state.resume_retry(ticket).is_none());
    assert!(!state.loading);
    assert!(state.peer().is_some());
}

#[test]
fn localized_peer_name_prefers_local_name_before_english_product_name() {
    let peer: ReceiverPeer = serde_json::from_value(serde_json::json!({
        "productId": 183, "productName": {"en":"English product"},
        "name": {"zh-CN":"本地名称", "en":"English name"}
    }))
    .unwrap();
    assert_eq!(peer.label("ZH-cn"), "本地名称");
    assert_eq!(peer.label("fr"), "English product");
}

#[gpui_kit::test]
fn parent_widget_mounts_temporary_loading_then_disconnected_binding(
    cx: &mut gpui_kit::TestAppContext,
) {
    use super::super::SourceControls;
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, px, size};
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut owner = None;
    let handle = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| SourceControls::new(179, window, cx));
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    let owner = owner.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        owner.update(cx, |view, cx| {
            view.receiver.page = queried(
                peer(Some(2), "MOUSE"),
                ReceiverDevicesObservation::complete(vec![(183, 1)]),
            );
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("receiver-parent-empty").is_none());
        assert!(window.find("receiver-parent-binding").visible());
        assert!(window.find("receiver-parent-loading").visible());
        assert_eq!(window.find("receiver-paired-name").disabled(), Some(false));
        assert_eq!(window.find("receiver-parent-unpair").disabled(), Some(true));
        assert_eq!(
            window.find("receiver-configure-device").disabled(),
            Some(true)
        );
        owner.update(cx, |view, cx| {
            view.observe_receiver_devices(ReceiverDevicesObservation::partial(vec![(183, 2)]), cx);
        });
        window.render_frame(cx);
        assert!(window.try_find("receiver-parent-loading").is_none());
        assert_eq!(
            window.find("receiver-configure-device").disabled(),
            Some(false)
        );
        owner.update(cx, |view, cx| {
            view.observe_receiver_devices(ReceiverDevicesObservation::partial(vec![(653, 0)]), cx)
        });
        window.render_frame(cx);
        assert!(window.find("receiver-parent-binding").visible());
        assert_eq!(window.find("receiver-paired-name").disabled(), Some(true));
        assert_eq!(
            window.find("receiver-configure-device").disabled(),
            Some(true)
        );
        owner.update(cx, |view, cx| {
            view.observe_receiver_devices(ReceiverDevicesObservation::complete(vec![]), cx)
        });
        window.render_frame(cx);
        assert!(window.find("receiver-parent-binding").visible());
        assert_eq!(window.find("receiver-paired-name").disabled(), Some(true));
        assert_eq!(
            window.find("receiver-parent-unpair").disabled(),
            Some(false)
        );
        assert_eq!(
            window.find("receiver-configure-device").disabled(),
            Some(true)
        );
    })
    .unwrap();
}

use super::callbacks::{self, Callback, ClutchRelease, DpiRuntime, ScanCode, StageAction};
use super::{Connection, Publication, State};
use crate::features::mouse_polling::MousePollingScope;
use gpui_kit::{AppContext, EntityId, TestAppContext};
use razer_device::{
    mouse_current_dpi::DpiPair,
    mouse_dpi_stages::{DpiStage, DpiStagesDraft},
};
use serde_json::json;

fn scope(revision: u64) -> MousePollingScope {
    MousePollingScope::new(EntityId::from(1), revision, 3)
}

#[test]
fn publication_failure_allows_identical_document_retry_and_stale_ack_cannot_accept_it() {
    let mut state = State::default();
    let document = json!({"activeProfile":"profile-a"});
    state.document = Some(document.clone());
    state.requested = Some(Publication {
        revision: 5,
        scope: scope(1),
        inter_otfs: false,
    });
    assert!(state.admitted(&document, scope(1)));
    state.acknowledge(5, Err("publication failed".into()), Some(scope(1)));
    assert!(!state.admitted(&document, scope(1)));
    state.requested = Some(Publication {
        revision: 6,
        scope: scope(1),
        inter_otfs: false,
    });
    state.acknowledge(5, Ok(()), Some(scope(1)));
    assert!(state.published.is_none());
    state.acknowledge(6, Ok(()), Some(scope(1)));
    assert!(state.error.is_none());
    assert_eq!(state.published.unwrap().revision, 6);
}

#[test]
fn same_container_reconnection_invalidates_epoch_and_old_profile_ack() {
    let mut state = State::default();
    let connection = |generation| {
        Some(Connection {
            container: "receiver".into(),
            generation,
        })
    };
    assert!(state.install_connection(connection(1)));
    let epoch = state.epoch;
    let publication = Publication {
        revision: 2,
        scope: scope(1),
        inter_otfs: false,
    };
    state.published = Some(publication);
    assert!(state.accepts_events(epoch, Some(publication), Some(scope(1))));
    assert!(!state.accepts_events(epoch, Some(publication), Some(scope(2))));
    assert!(state.install_connection(connection(2)));
    assert!(!state.accepts_events(epoch, Some(publication), Some(scope(1))));
    assert_eq!(state.epoch, epoch + 1);
}

#[test]
fn event_gate_preserves_held_release_during_same_scope_refresh_and_rechecks_stop() {
    let mut state = State::default();
    let publication = Publication {
        revision: 2,
        scope: scope(1),
        inter_otfs: false,
    };
    state.published = Some(publication);
    assert!(state.accepts_events(0, Some(publication), Some(scope(1))));
    state.requested = Some(Publication {
        revision: 3,
        ..publication
    });
    assert!(state.accepts_events(0, Some(publication), Some(scope(1))));
    state.requested = None;
    state.stopping = true;
    assert!(!state.accepts_events(0, Some(publication), Some(scope(1))));
}

#[test]
fn callback_boundary_accepts_native_event_and_rejects_missing_clutch_axes() {
    let raw = |group| json!({"deviceEventType":2,"jsonEvent":{"type":"unsupportedmapping","input":"{\"scancode\":5}","output":json!({"type":"sensitivity","flag":0,"sensitivityGroup":group}).to_string()}});
    let callback = callbacks::parse(
        raw(json!({"sensitivityAssignment":"DPI_Clutch","x":900,"y":1200})),
        100,
        30000,
    )
    .unwrap();
    assert!(matches!(
        callback,
        Callback::ClutchDown {
            dpi: DpiPair { x: 900, y: 1200 },
            ..
        }
    ));
    assert!(
        callbacks::parse(
            raw(json!({"sensitivityAssignment":"DPI_Clutch","x":900})),
            100,
            30000
        )
        .is_err()
    );
    assert!(
        callbacks::parse(
            raw(json!({"sensitivityAssignment":"DPI_Clutch","x":99,"y":1200})),
            100,
            30000
        )
        .is_err()
    );
}

#[test]
fn callback_forwarding_preserves_native_timer_and_source_linked_chroma_branches() {
    let raw = |output| json!({"input":"{\"scancode\":5}","output":serde_json::to_string(&output).unwrap()});
    for id in [json!("linked"), json!(""), json!(null)] {
        let callback = callbacks::parse_raw(
            raw(json!({"type":"switchProfile","guid":"next", "lightpacId":id})),
            100,
            30000,
        )
        .unwrap();
        assert!(matches!(
            callback,
            Callback::Navigate {
                lightpac: Some(_),
                ..
            }
        ));
    }
    for output in [
        json!({"type":"switchProfile","guid":"next"}),
        json!({"type":"navigateProfile","name":"CycleUp", "lightpacId":"linked"}),
    ] {
        let callback = callbacks::parse_raw(raw(output), 100, 30000).unwrap();
        assert!(matches!(
            callback,
            Callback::Navigate { lightpac: None, .. }
        ));
    }
    let mut inter = raw(json!({"type":"switchProfile","guid":"next","lightpacId":"linked"}));
    inter["eventName"] = json!("interDevice");
    assert!(matches!(
        callbacks::parse_raw(inter, 100, 30000).unwrap(),
        Callback::Navigate { lightpac: None, .. }
    ));
    let mut native = raw(json!({"type":"lightpac", "id":"linked"}));
    native["type"] = json!("unsupportedmapping");
    let Callback::Forward(forwarded) = callbacks::parse(
        json!({"deviceEventType":2,"jsonEvent":native,"timeTick":8142}),
        100,
        30000,
    )
    .unwrap() else {
        panic!("native Chroma callback must forward")
    };
    assert_eq!(forwarded["timerTick"], 8142);
}

#[gpui_kit::test]
fn connection_replacement_cancels_retained_callback_pump_without_a_window(cx: &mut TestAppContext) {
    let state = cx.new(|_| State::default());
    let (completion, receive) = async_channel::bounded::<()>(1);
    state.update(cx, |state, cx| {
        state.install_connection(Some(Connection {
            container: "receiver".into(),
            generation: 1,
        }));
        state.pump = Some(cx.spawn(async move |_, _| {
            let _ = receive.recv().await;
        }));
    });
    cx.run_until_parked();
    assert!(!completion.is_closed());
    state.update(cx, |state, _| {
        state.install_connection(Some(Connection {
            container: "receiver".into(),
            generation: 2,
        }));
        assert!(state.pump.is_none());
        assert!(state.published.is_none());
    });
    cx.run_until_parked();
    assert!(
        completion.is_closed(),
        "the predecessor callback task must release its queue receiver"
    );
}

fn shared_saved_publication(cx: &mut TestAppContext, mounted: u32, source: u32) {
    use super::{MouseProductWorkspace, ReceiverMappingPublication};
    use gpui_kit::{component::Root, px, size};
    cx.update(gpui_kit::init);
    let mut page = None;
    let window = cx.open_window(size(px(1000.), px(700.)), |window, cx| {
        let view = cx.new(|cx| MouseProductWorkspace::new(mounted, window, cx));
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.unwrap();
    let events = cx.new(|_| Vec::<ReceiverMappingPublication>::new());
    let _subscription = events.update(cx, |_, cx| {
        cx.subscribe(&page, |events, _, event: &ReceiverMappingPublication, _| {
            events.push(event.clone())
        })
    });
    let first = uuid::Uuid::new_v4();
    let second = uuid::Uuid::new_v4();
    let document = json!({"productId":source,"activeProfile":"saved-profile","version":27});
    cx.update_window(window.into(), |_, window, cx| {
        page.update(cx, |page, cx| {
            let mut profile = page.snapshot();
            profile["guid"] = json!("saved-profile");
            page.restore(Some(&profile), window, cx);
            page.configure_shared_mapping(first, 191, "receiver".into(), 4);
            page.publish_receiver_mapping("receiver".into(), document.clone(), cx);
            assert!(page.receiver_mapping.owner.is_none());
            assert!(page.receiver_mapping.pump.is_none());
            assert!(page.receiver_mapping.published.is_none());
        });
    })
    .unwrap();
    cx.run_until_parked();
    let request = events.read_with(cx, |events, _| {
        assert_eq!(events.len(), 1);
        events[0].clone()
    });
    assert_eq!(request.owner(), first);
    assert_eq!(request.product_id(), source);
    let delivery = request.for_mounted_product(mounted).unwrap();
    assert_eq!(delivery.product_id(), mounted);
    assert_eq!(delivery.document(), &document);
    assert_eq!(request.document(), &document);
    page.update(cx, |page, cx| {
        assert!(!page.observe_shared_mapping_publication(second, request.revision(), Ok(()), cx));
        assert!(page.receiver_mapping.published.is_none());
        assert!(page.observe_shared_mapping_publication(first, request.revision(), Ok(()), cx));
        assert_eq!(
            page.receiver_mapping.published.unwrap().revision,
            request.revision()
        );
        assert!(page.observe_shared_mapping_event(
            first,
            json!({"source":"devicemodechanged","eventType":4,"event":{"isEnabled":true}}),
            cx
        ));
        assert_eq!(page.receiver_mapping.dpi.native_otfs, Some(true));
        page.configure_shared_mapping(second, 191, "receiver".into(), 5);
        assert!(!page.observe_shared_mapping_publication(first, request.revision(), Ok(()), cx));
        assert!(!page.observe_shared_mapping_event(
            first,
            json!({"source":"devicemodechanged","eventType":4,"event":{"isEnabled":false}}),
            cx
        ));
        page.release_shared_mapping(first, Some("old cleanup".into()), cx);
        assert!(page.shared_mapping_matches(191, "receiver", 5));
        page.release_shared_mapping(second, None, cx);
        assert!(!page.shared_mapping_connection_matches("receiver", 5));
        assert!(page.receiver_mapping.published.is_none());
    });
}

#[gpui_kit::test]
fn shared_receiver_saved_publication_keeps_page_owner_and_rejects_stale_completion(
    cx: &mut TestAppContext,
) {
    shared_saved_publication(cx, 190, 190);
}

#[gpui_kit::test]
fn shared_receiver_mounted105_uses_source104_without_changing_page_identity(
    cx: &mut TestAppContext,
) {
    shared_saved_publication(cx, 105, 104);
}

#[test]
fn nested_clutch_release_restores_last_held_value_then_profile() {
    let mut state = DpiRuntime::default();
    let first = DpiPair { x: 400, y: 500 };
    let second = DpiPair { x: 800, y: 900 };
    state.push_clutch(ScanCode::Number(1), first);
    state.push_clutch(ScanCode::Number(2), second);
    assert!(
        matches!(state.release_clutch(&ScanCode::Number(2)), ClutchRelease::Temporary(dpi) if dpi == first)
    );
    assert!(state.clutched());
    assert!(matches!(
        state.release_clutch(&ScanCode::Number(1)),
        ClutchRelease::Profile
    ));
    assert!(!state.clutched());
}

#[test]
fn temporary_dpi_restoration_tracks_physical_connection_independently_of_mapping_owner() {
    let connection = |generation| {
        Some(Connection {
            container: "receiver".into(),
            generation,
        })
    };
    let mut restoration = callbacks::TemporaryRestoration::default();
    restoration.observe_connection(connection(1));
    restoration.require();
    restoration.observe_connection(connection(1));
    assert!(restoration.required());
    let mut mapping = State::default();
    mapping.stop();
    assert!(restoration.required());
    restoration.observe_connection(connection(2));
    assert!(!restoration.required());
}

#[test]
fn stage_navigation_skips_hidden_rows_wraps_only_cycles_and_keeps_disabled_selection() {
    let mut stages = DpiStagesDraft {
        enabled: true,
        active_stage: 3,
        stages: [400, 800, 1600]
            .into_iter()
            .enumerate()
            .map(|(index, x)| DpiStage {
                x,
                y: x,
                visible: index != 1,
                independent: false,
            })
            .collect(),
    };
    assert_eq!(callbacks::next_stage(&stages, StageAction::Up), Some(3));
    assert_eq!(
        callbacks::next_stage(&stages, StageAction::CycleUp),
        Some(1)
    );
    assert_eq!(callbacks::next_stage(&stages, StageAction::Down), Some(1));
    stages.enabled = false;
    assert_eq!(
        callbacks::next_stage(&stages, StageAction::CycleUp),
        Some(3)
    );
    assert_eq!(callbacks::otfs_value(29975, true, 50, 100, 30000), 30000);
    assert_eq!(callbacks::otfs_value(125, false, 50, 100, 30000), 100);
}

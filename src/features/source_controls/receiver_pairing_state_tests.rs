use super::*;

fn ready() -> (PairingState, u64) {
    let mut state = PairingState::default();
    let session = state.open().session();
    state.observe(ReceiverPairingObservation::bindings(session, vec![]));
    state.observe(ReceiverPairingObservation::firmware_version(
        session,
        Some("1.2.3".into()),
    ));
    (state, session)
}

#[test]
fn unsent_intent_does_not_report_scanning_or_pairing() {
    let (mut state, session) = ready();
    let event = state.scan(ReceiverCategory::Mouse).unwrap();
    assert!(matches!(
        event.intent(),
        ReceiverPairingIntent::Scan(ReceiverCategory::Mouse)
    ));
    assert_eq!(state.status, Status::Ready);
    state.cancel_pending();
    assert!(
        !state
            .observe(ReceiverPairingObservation::progress(
                session,
                ReceiverProgress::Scanning(ReceiverCategory::Mouse)
            ))
            .0
    );
    assert_eq!(state.status, Status::Ready);
}

#[test]
fn closed_and_previous_sessions_cannot_publish_results() {
    let (mut state, old_session) = ready();
    state.close();
    assert!(
        !state
            .observe(ReceiverPairingObservation::bindings(
                old_session,
                vec![ReceiverPeer::queried(183, 1)]
            ))
            .0
    );
    let new_session = state.open().session();
    assert_ne!(new_session, old_session);
    assert!(
        !state
            .observe(ReceiverPairingObservation::bound(
                old_session,
                ReceiverPeer::queried(183, 1)
            ))
            .0
    );
    assert_eq!(state.status, Status::Loading);
    assert!(state.bound.is_empty());
}

#[test]
fn cancel_unpair_keeps_observed_binding_and_cancels_local_intent() {
    let (mut state, session) = ready();
    state.observe(ReceiverPairingObservation::bindings(
        session,
        vec![ReceiverPeer::queried(183, 0)],
    ));
    state.confirm_unpair();
    assert_eq!(state.status, Status::ConfirmUnpair);
    assert!(matches!(
        state.unbind().unwrap().intent(),
        ReceiverPairingIntent::Unbind(183)
    ));
    assert_eq!(state.status, Status::ConfirmUnpair);
    state.cancel_unpair();
    assert_eq!(state.status, Status::Paired);
    assert_eq!(state.bound[0].status(), Some(0));
    assert!(state.pending.is_none());
}

#[test]
fn sole_real_candidate_creates_intent_without_fabricating_success() {
    let (mut state, session) = ready();
    let candidate: ReceiverPeer = serde_json::from_value(serde_json::json!({
        "productId":183,"category":"MOUSE","serialNumber":"observed-id","vendorSpecific":17
    }))
    .unwrap();
    let (_, event) = state.observe(ReceiverPairingObservation::scanned(
        session,
        vec![candidate],
    ));
    let ReceiverPairingIntent::Bind(peer) = event.unwrap().intent().clone() else {
        panic!("expected bind intent")
    };
    assert_eq!(serde_json::to_value(peer).unwrap()["vendorSpecific"], 17);
    assert_eq!(state.status, Status::Scanned);
    assert!(state.bound.is_empty());
}

#[test]
fn hardware_pid_status_does_not_manufacture_metadata_or_firmware() {
    let peer = ReceiverPeer::queried(183, 1);
    assert_eq!(peer.product_id(), 183);
    assert_eq!(peer.status(), Some(1));
    assert_eq!(peer.category(), None);
    assert_eq!(peer.image_identity(), None);
    assert_eq!(peer.label("zh-cn"), "PID 183");
    let mut state = PairingState::default();
    let session = state.open().session();
    state.observe(ReceiverPairingObservation::bindings(session, vec![]));
    assert!(state.scan(ReceiverCategory::Mouse).is_none());
    assert_eq!(state.status, Status::Ready);
}

#[test]
fn binding_read_failure_is_kept_distinct_from_a_successful_empty_list() {
    let mut state = PairingState::default();
    let session = state.open().session();
    state.observe(ReceiverPairingObservation::failed(
        session,
        ReceiverOperation::Bindings,
    ));
    assert_eq!(state.failure, Some(ReceiverOperation::Bindings));
    assert_eq!(state.status, Status::Ready); // se's actual error presentation branch.
    state.refresh();
    assert!(matches!(
        state.pending,
        Some(ReceiverPairingIntent::QueryBindings)
    ));
    state.observe(ReceiverPairingObservation::bindings(session, vec![]));
    assert!(state.failure.is_none());
}

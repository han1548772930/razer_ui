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
    let (mut state, _) = ready();
    let event = state.scan(ReceiverCategory::Mouse).unwrap();
    let session = event.session();
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
    let (mut state, _) = ready();
    let session = state.refresh().unwrap().session();
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
    let (mut state, _) = ready();
    let session = state.scan(ReceiverCategory::Mouse).unwrap().session();
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
    let session = state.refresh().unwrap().session();
    assert!(matches!(
        state.pending,
        Some(ReceiverPairingIntent::QueryBindings)
    ));
    state.observe(ReceiverPairingObservation::bindings(session, vec![]));
    assert!(state.failure.is_none());
}

#[test]
fn cancelled_operations_reject_delayed_success_and_failure() {
    let (mut state, _) = ready();
    let scan = state.scan(ReceiverCategory::Mouse).unwrap();
    let cancel = state.cancel_pending().unwrap();
    assert_ne!(scan.session(), cancel.session());
    assert!(matches!(cancel.intent(), ReceiverPairingIntent::Cancel));
    assert!(
        !state
            .observe(ReceiverPairingObservation::scanned(
                scan.session(),
                vec![ReceiverPeer::queried(183, 1)],
            ))
            .0
    );
    assert!(
        !state
            .observe(ReceiverPairingObservation::failed(
                scan.session(),
                ReceiverOperation::Scan,
            ))
            .0
    );
    assert!(state.pending.is_none());
    assert!(state.candidates.is_empty());
    assert!(state.failure.is_none());
}

#[test]
fn refreshed_queries_reject_older_results_and_wrong_operation_results() {
    let (mut state, _) = ready();
    let old = state.refresh().unwrap().session();
    let current = state.refresh().unwrap().session();
    assert_ne!(old, current);
    assert!(
        !state
            .observe(ReceiverPairingObservation::bindings(
                old,
                vec![ReceiverPeer::queried(183, 1)],
            ))
            .0
    );
    assert!(
        !state
            .observe(ReceiverPairingObservation::unbound(current))
            .0
    );
    assert!(matches!(
        state.pending,
        Some(ReceiverPairingIntent::QueryBindings)
    ));
    assert!(
        state
            .observe(ReceiverPairingObservation::bindings(current, vec![]))
            .0
    );
}

#[test]
fn progress_keeps_operation_category_and_single_candidate_advances_generation() {
    let (mut state, _) = ready();
    let scan = state.scan(ReceiverCategory::Keyboard).unwrap().session();
    for progress in [
        ReceiverProgress::Upgrading(ReceiverCategory::Keyboard),
        ReceiverProgress::Scanning(ReceiverCategory::Keyboard),
    ] {
        assert!(
            state
                .observe(ReceiverPairingObservation::progress(scan, progress))
                .0
        );
        assert!(state.pending.is_none());
    }
    let (accepted, bind) = state.observe(ReceiverPairingObservation::scanned(
        scan,
        vec![ReceiverPeer::queried(183, 1)],
    ));
    assert!(accepted);
    let bind = bind.unwrap();
    assert_ne!(scan, bind.session());
    assert_eq!(state.category, Some(ReceiverCategory::Keyboard));
    assert!(
        !state
            .observe(ReceiverPairingObservation::scanned(scan, vec![]))
            .0
    );
    let cancelled = state.cancel_pending().unwrap();
    assert!(
        !state
            .observe(ReceiverPairingObservation::bound(
                bind.session(),
                ReceiverPeer::queried(183, 1),
            ))
            .0
    );
    assert_ne!(cancelled.session(), bind.session());
    assert!(state.bound.is_empty());
}

#[test]
fn candidate_refresh_keeps_selected_index_within_new_list() {
    let (mut state, _) = ready();
    let first = state.scan(ReceiverCategory::Mouse).unwrap().session();
    state.observe(ReceiverPairingObservation::scanned(
        first,
        (181..184).map(|id| ReceiverPeer::queried(id, 1)).collect(),
    ));
    state.select(2);
    let second = state.scan(ReceiverCategory::Mouse).unwrap().session();
    state.observe(ReceiverPairingObservation::scanned(
        second,
        (181..183).map(|id| ReceiverPeer::queried(id, 1)).collect(),
    ));
    assert_eq!(state.selected, 1);
    let event = state.bind().unwrap();
    assert!(
        matches!(event.intent(), ReceiverPairingIntent::Bind(peer) if peer.product_id() == 182)
    );
}

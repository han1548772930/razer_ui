use super::{MouseLegacyDpiRequested, MousePollingScope, State};
use gpui_kit::EntityId;
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};

fn scope(generation: u64) -> MousePollingScope {
    MousePollingScope::new(EntityId::from(1), 2, generation)
}

fn sensitivity() -> Value {
    json!({"State":true,"Active":2,"Stages":{"DPIStage":[
        {"Active":true,"X":400,"Y":500}, {"Active":true,"X":1600,"Y":1700}
    ]}})
}

fn request(sequence: u64) -> MouseLegacyDpiRequested {
    MouseLegacyDpiRequested {
        scope: scope(3),
        sequence,
        sensitivity: sensitivity(),
        send: true,
        persist: false,
        dpi: Some((400, 500)),
        cleanup: false,
        restores_temporary: false,
        restoration_ticket: 0,
        valid: Arc::new(AtomicBool::new(true)),
    }
}

#[test]
fn legacy_dpi_exit_restores_profile_after_worker_cleanup_and_preserves_requested_save() {
    let mut state = State::default();
    let old = request(1);
    assert!(state.enqueue(old.clone()).is_some());
    state.temporary_restoration.require();
    let mut saved = request(2);
    saved.persist = true;
    assert!(state.enqueue(saved.clone()).is_none());
    state.sequence = 2;
    assert!(
        state
            .prepare_exit(Some(scope(3)), sensitivity(), true)
            .unwrap()
            .is_none()
    );
    assert!(!old.is_valid());
    assert!(!saved.is_valid());
    assert!(state.owns_completion(&old));
    let restore = state.finish(&old, Some(scope(3)), false).unwrap();
    assert_eq!(restore.sequence, 3);
    assert!(restore.cleanup);
    assert!(restore.persist);
    assert!(restore.send);
    assert_eq!(restore.dpi, Some((1600, 1700)));
    assert_ne!(restore.dpi, old.dpi);
    assert!(state.temporary_restoration.required());
    assert!(state.finish(&old, Some(scope(3)), true).is_none());
    assert!(state.owns_completion(&restore));
    state.finish(&restore, Some(scope(3)), true);
    assert!(!state.temporary_restoration.required());
}

#[test]
fn legacy_dpi_exit_without_pending_never_persists_temporary_dpi_or_accepts_failed_restore() {
    let mut state = State::default();
    state.temporary_restoration.require();
    let restore = state
        .prepare_exit(Some(scope(3)), sensitivity(), true)
        .unwrap()
        .unwrap();
    assert!(!restore.persist);
    assert_eq!(restore.dpi, Some((1600, 1700)));
    state.finish(&restore, Some(scope(3)), false);
    assert!(state.temporary_restoration.required());
    let retry = state
        .prepare_exit(Some(scope(3)), sensitivity(), true)
        .unwrap()
        .unwrap();
    assert!(retry.is_valid());
    state.finish(&retry, Some(scope(3)), true);
    assert!(!state.temporary_restoration.required());
}

#[test]
fn legacy_dpi_exit_rejects_missing_connection_and_canceled_receipts() {
    let mut state = State::default();
    state.temporary_restoration.require();
    assert!(state.prepare_exit(None, sensitivity(), true).is_err());
    let restore = state
        .prepare_exit(Some(scope(3)), sensitivity(), true)
        .unwrap()
        .unwrap();
    state.cancel();
    state.finish(&restore, Some(scope(3)), true);
    assert!(state.temporary_restoration.required());
    let newer = state
        .prepare_exit(Some(scope(4)), sensitivity(), true)
        .unwrap()
        .unwrap();
    state.finish(&restore, Some(scope(4)), true);
    assert!(state.owns_completion(&newer));
    assert!(newer.is_valid());
}
#[test]
fn legacy_dpi_profile_ack_cannot_clear_a_later_temporary_write() {
    let mut state = State::default();
    let mut old = request(1);
    old.restores_temporary = true;
    state.enqueue(old.clone());
    state.temporary_restoration.require();
    assert!(state.finish(&old, Some(scope(3)), true).is_none());
    assert!(state.temporary_restoration.required());
    let restore = state
        .prepare_exit(Some(scope(3)), sensitivity(), true)
        .unwrap()
        .unwrap();
    state.finish(&restore, Some(scope(3)), true);
    assert!(!state.temporary_restoration.required());
}

use super::selected_dpi;
#[test]
fn active_xy_uses_source_selection_with_stages_on_or_off() {
    let mut value = json!({"Active":2,"State":true,"Stages":{"DPIStage":[
        {"Active":false,"X":800,"Y":800}, {"Active":true,"X":1600,"Y":3200}]}});
    assert_eq!(selected_dpi(&value), Some((1600, 3200)));
    value["State"] = false.into();
    assert_eq!(selected_dpi(&value), Some((1600, 3200)));
    value["Active"] = 1.into();
    assert_eq!(selected_dpi(&value), None);
    value["Active"] = 0.into();
    assert_eq!(selected_dpi(&value), None);
}
#[test]
fn invalid_dpi_cannot_become_a_device_request() {
    let value = json!({"Active":1,"Stages":{"DPIStage":[{"Active":true,"X":99,"Y":800}]}});
    assert_eq!(selected_dpi(&value), None);
}

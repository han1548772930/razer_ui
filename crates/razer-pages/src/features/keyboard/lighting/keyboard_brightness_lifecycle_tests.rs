use super::State;
use std::sync::{Arc, atomic::Ordering};

#[test]
fn connection_invalidation_cancels_io_but_retains_its_serial_slot() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    let guard = state.cancellation_guard(read.generation(), None).unwrap();
    state.invalidate();

    assert!(guard.load(Ordering::Acquire));
    assert!(state.cancellation_guard(read.generation(), None).is_none());
    assert!(state.begin_write(35).is_none());
    assert_eq!(state.queued, Some(35));
    assert_eq!(state.read_pending.unwrap().generation(), read.generation());
    assert!(state.begin_read().is_none());
}

#[test]
fn slider_edits_reject_the_old_read_without_canceling_the_queued_latest_write() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    let guard = state.cancellation_guard(read.generation(), None).unwrap();
    state.preview_edit();
    assert!(state.begin_write(40).is_none());
    assert!(state.begin_write(75).is_none());

    assert!(!guard.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(
        &guard,
        &state.cancellation_guard(read.generation(), None).unwrap()
    ));
    assert_ne!(state.edit_revision, read.edit_revision);
    assert_eq!(state.queued, Some(75));
    assert_eq!(state.read_pending.unwrap().generation(), read.generation());
}

#[test]
fn leaving_the_active_device_cancels_the_pending_setter_without_another_worker() {
    let mut state = State::default();
    state.set_active(true);
    let write = state.begin_write(60).unwrap();
    let guard = state
        .cancellation_guard(write.generation(), Some(60))
        .unwrap();
    state.set_active(false);
    state.set_active(true);

    assert!(guard.load(Ordering::Acquire));
    assert!(state.begin_read().is_none());
    assert!(state.begin_write(20).is_none());
    assert_eq!(state.pending.unwrap().generation(), write.generation());
    assert_eq!(state.queued, Some(20));
    assert!(
        state
            .cancellation_guard(write.generation(), Some(60))
            .is_none()
    );
}

#[test]
fn releasing_a_page_cancels_its_request_and_a_new_owner_has_a_distinct_guard() {
    let mut first = State::default();
    let write = first.begin_write(55).unwrap();
    let old = first
        .cancellation_guard(write.generation(), Some(55))
        .unwrap();
    drop(first);

    let mut replacement = State::default();
    let next = replacement.begin_write(55).unwrap();
    let current = replacement
        .cancellation_guard(next.generation(), Some(55))
        .unwrap();
    assert_eq!(next.generation(), write.generation());
    assert!(old.load(Ordering::Acquire));
    assert!(!current.load(Ordering::Acquire));
    assert!(!Arc::ptr_eq(&old, &current));
}

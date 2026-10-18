use super::{Observation, Request, State};
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
    assert_eq!(state.queued.map(|edit| edit.percent), Some(35));
    assert_eq!(state.read_pending.unwrap().generation(), read.generation());
    assert!(state.begin_read().is_none());
}

#[test]
fn slider_edits_preserve_the_old_read_slot_and_the_queued_latest_write() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    let guard = state.cancellation_guard(read.generation(), None).unwrap();
    state.preview_edit(30);
    assert!(state.begin_write(40).is_none());
    assert!(state.begin_write(75).is_none());

    assert!(!guard.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(
        &guard,
        &state.cancellation_guard(read.generation(), None).unwrap()
    ));
    assert_eq!(state.displayed_value(), Some(75));
    assert_eq!(state.queued.map(|edit| edit.percent), Some(75));
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
    assert_eq!(state.queued.map(|edit| edit.percent), Some(20));
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

#[test]
fn actual_reading_42_reaches_the_original_control_value_without_a_profile_default() {
    let mut state = State::default();
    assert_eq!(state.displayed_value(), None);
    state.set_active(true);
    let read = state.begin_read().unwrap();
    assert_eq!(state.displayed_value(), None);

    assert!(
        state
            .finish_read(read.generation(), Some(42), None, true)
            .is_none()
    );
    assert_eq!(state.observation, Observation::Confirmed(42));
    assert_eq!(state.displayed_value(), Some(42));
    assert_ne!(state.displayed_value(), Some(100));
    assert!(state.preview.is_none());
}

#[test]
fn a_valid_reading_remains_visible_with_a_separate_cleanup_failure() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    state.finish_read(
        read.generation(),
        Some(42),
        Some("shutdown failed".into()),
        true,
    );

    assert_eq!(state.observation, Observation::Confirmed(42));
    assert_eq!(state.displayed_value(), Some(42));
    assert_eq!(state.error.as_deref(), Some("shutdown failed"));
}

#[test]
fn a_failed_refresh_hides_the_last_value_and_retry_can_publish_a_new_reading() {
    let mut state = State::default();
    state.set_active(true);
    let first = state.begin_read().unwrap();
    state.finish_read(first.generation(), Some(42), None, true);
    let refresh = state.begin_read().unwrap();
    state.finish_read(refresh.generation(), None, Some("GET failed".into()), true);

    assert_eq!(state.observation, Observation::Failed);
    assert_eq!(state.displayed_value(), None);
    assert_eq!(state.error.as_deref(), Some("GET failed"));

    let retry = state.begin_read().unwrap();
    state.finish_read(retry.generation(), Some(37), None, true);
    assert_eq!(state.displayed_value(), Some(37));
    assert!(state.error.is_none());
}

#[test]
fn unknown_protocol_values_are_retained_without_becoming_a_clamped_thumb() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    state.finish_read(read.generation(), Some(153), None, true);

    assert_eq!(state.observation, Observation::Unknown(153));
    assert_eq!(state.displayed_value(), None);
}

#[test]
fn an_actual_zero_is_a_confirmed_value_and_not_an_unobserved_sentinel() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    state.finish_read(read.generation(), Some(0), None, true);

    assert_eq!(state.observation, Observation::Confirmed(0));
    assert_eq!(state.displayed_value(), Some(0));
}

#[test]
fn disconnect_and_reconnect_wait_for_cleanup_and_reject_the_old_reading() {
    let mut state = State::default();
    state.set_active(true);
    let old = state.begin_read().unwrap();
    let guard = state.cancellation_guard(old.generation(), None).unwrap();
    state.disconnect();
    state.set_active(false);
    assert_eq!(state.observation, Observation::Disconnected);
    assert_eq!(state.displayed_value(), None);
    assert!(guard.load(Ordering::Acquire));

    state.set_active(true);
    assert_eq!(state.observation, Observation::Unobserved);
    assert!(state.begin_read().is_none());
    let Some(Request::Read(current)) = state.finish_read(old.generation(), Some(100), None, false)
    else {
        panic!("The reconnect getter must retain the old worker's cleanup barrier");
    };
    assert_eq!(state.displayed_value(), None);
    assert_eq!(
        state.read_pending.unwrap().generation(),
        current.generation()
    );
    assert!(
        state
            .finish_read(old.generation(), Some(100), None, true)
            .is_none()
    );
    assert_eq!(
        state.read_pending.unwrap().generation(),
        current.generation()
    );
    state.finish_read(current.generation(), Some(38), None, true);
    assert_eq!(state.displayed_value(), Some(38));
}

#[test]
fn an_old_getter_updates_the_actual_observation_without_overwriting_a_new_preview() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    state.preview_edit(75);
    state.finish_read(read.generation(), Some(42), None, true);

    assert_eq!(state.observation, Observation::Confirmed(42));
    assert_eq!(state.displayed_value(), Some(75));
    assert_eq!(state.preview.unwrap().percent, 75);
}

#[test]
fn an_old_setter_readback_does_not_replace_a_new_unreleased_preview() {
    let mut state = State::default();
    let write = state.begin_write(40).unwrap();
    state.preview_edit(75);
    assert!(
        state
            .finish_write(write.generation(), 40, Some(40), None, true)
            .is_none()
    );

    assert_eq!(state.observation, Observation::Confirmed(40));
    assert_eq!(state.displayed_value(), Some(75));
    assert_eq!(state.preview.unwrap().percent, 75);
}

#[test]
fn queued_write_keeps_its_commit_revision_when_a_new_drag_starts() {
    let mut state = State::default();
    state.set_active(true);
    let read = state.begin_read().unwrap();
    assert!(state.begin_write(40).is_none());
    assert!(state.begin_write(75).is_none());
    state.preview_edit(90);

    let Some(Request::Write(write)) = state.finish_read(read.generation(), Some(42), None, true)
    else {
        panic!("The latest committed write must follow the getter");
    };
    assert_eq!(write.percent(), 75);
    assert_eq!(state.displayed_value(), Some(90));
    state.finish_write(write.generation(), 75, Some(75), None, true);
    assert_eq!(state.observation, Observation::Confirmed(75));
    assert_eq!(state.displayed_value(), Some(90));

    let release = state.begin_write(90).unwrap();
    state.finish_write(release.generation(), 90, Some(90), None, true);
    assert_eq!(state.observation, Observation::Confirmed(90));
    assert_eq!(state.displayed_value(), Some(90));
    assert!(state.preview.is_none());
}

#[test]
fn a_failed_setter_keeps_the_explicit_unconfirmed_edit_without_reporting_an_actual_value() {
    let mut state = State::default();
    let write = state.begin_write(75).unwrap();
    state.finish_write(
        write.generation(),
        75,
        None,
        Some("SET failed".into()),
        true,
    );

    assert_eq!(state.observation, Observation::Failed);
    assert_eq!(state.error.as_deref(), Some("SET failed"));
    assert_eq!(state.displayed_value(), Some(75));
    assert_eq!(state.preview.unwrap().percent, 75);
}

#[test]
fn a_confirmed_setter_replaces_its_preview_and_preserves_cleanup_failure_separately() {
    let mut state = State::default();
    let write = state.begin_write(55).unwrap();
    state.finish_write(
        write.generation(),
        55,
        Some(55),
        Some("termination failed".into()),
        true,
    );

    assert_eq!(state.observation, Observation::Confirmed(55));
    assert_eq!(state.displayed_value(), Some(55));
    assert!(state.preview.is_none());
    assert_eq!(state.error.as_deref(), Some("termination failed"));
}

#[test]
fn a_refresh_requested_during_a_setter_runs_after_its_cleanup() {
    let mut state = State::default();
    state.set_active(true);
    let write = state.begin_write(60).unwrap();
    assert!(state.begin_read().is_none());

    let Some(Request::Read(read)) =
        state.finish_write(write.generation(), 60, Some(60), None, true)
    else {
        panic!("A busy getter request must be delivered after the setter");
    };
    state.finish_read(read.generation(), Some(62), None, true);
    assert_eq!(state.displayed_value(), Some(62));
}

#[test]
fn old_scope_completion_preserves_an_explicit_new_scope_write() {
    let mut state = State::default();
    state.set_active(true);
    let old = state.begin_read().unwrap();
    state.invalidate();
    assert!(state.begin_write(35).is_none());

    let Some(Request::Write(write)) = state.finish_read(old.generation(), Some(100), None, false)
    else {
        panic!("The successor's explicit write must survive the old scope's completion");
    };
    assert_eq!(write.percent(), 35);
    assert_eq!(state.observation, Observation::Unobserved);
    assert_eq!(state.displayed_value(), Some(35));
    state.finish_write(write.generation(), 35, Some(35), None, true);
    assert_eq!(state.displayed_value(), Some(35));
    assert!(state.preview.is_none());
}

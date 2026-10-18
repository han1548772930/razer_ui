use super::{DpiStage, DpiStagesDraft, MousePollingScope, ProfileInitialization, Selection, State};
use gpui_kit::EntityId;
use serde_json::json;
use std::sync::{Arc, atomic::AtomicBool};

const PROFILE_GUID: &str = "{3CD4A786-315B-44C8-91E0-1290FFEE3981}";

fn scope(connection_epoch: u64) -> MousePollingScope {
    MousePollingScope::new(EntityId::from(1), 2, connection_epoch)
}

fn source_profile() -> DpiStagesDraft {
    DpiStagesDraft {
        enabled: true,
        active_stage: 3,
        stages: [500, 1000, 1500, 2000, 2500]
            .into_iter()
            .map(|value| DpiStage {
                x: value,
                y: value,
                visible: true,
                independent: false,
            })
            .collect(),
    }
}

fn input(key: Option<&str>, connection_epoch: u64) -> ProfileInitialization<'static> {
    let stages = source_profile();
    ProfileInitialization {
        connection_key: key.map(str::to_owned),
        scope: Some(scope(connection_epoch)),
        profile_guid: PROFILE_GUID,
        selection: Selection::capture(182, &stages),
        stages: Some(stages),
        macro_recording: false,
    }
}

#[test]
fn current182_initialization_first_connection_keeps_initializer_valid_before_its_following_get() {
    let mut state = State::default();
    let initialize = state
        .initialize_connection(input(Some("first-route/profile"), 3))
        .expect("first connection dispatches its source initializer");
    assert!(initialize.is_valid());
    assert!(state.owns_completion(&initialize));
    assert_eq!(state.connection_key, initialize.connection_key);
    assert_eq!(state.connection_scope, Some(initialize.scope));
    let change = initialize.change().unwrap();
    assert!(change.initialize);
    assert_eq!(change.profile_guid.as_deref(), Some(PROFILE_GUID));
    assert!(change.active.is_none());
    assert!(!initialize.is_read());
    razer_device::mouse_current_dpi::validate_change_for(182, change).unwrap();
    assert!(!state.take_ready_refresh());

    state.finish_initialization(&initialize, true);
    assert!(state.release(&initialize));
    assert!(state.take_queued(Some(scope(3))).is_none());
    assert!(state.take_ready_refresh());
    assert!(!state.take_ready_refresh());
    assert_eq!(
        state.initialization_key.as_deref(),
        Some("first-route/profile")
    );
}

#[test]
fn current182_initialization_repeated_connection_does_not_cancel_or_repeat_initialization() {
    let mut state = State::default();
    let initialize = state
        .initialize_connection(input(Some("first-route/profile"), 3))
        .unwrap();
    for _ in 0..3 {
        assert!(
            state
                .initialize_connection(input(Some("first-route/profile"), 3))
                .is_none()
        );
        assert!(initialize.is_valid());
        assert!(state.owns_completion(&initialize));
        assert!(state.initialization_queued.is_none());
        assert!(state.queued.is_none());
    }
    state.finish_initialization(&initialize, true);
    assert!(state.release(&initialize));
    assert!(state.take_ready_refresh());
    assert!(
        state
            .initialize_connection(input(Some("first-route/profile"), 3))
            .is_none()
    );
    assert!(state.pending.is_none());
    assert!(!state.take_ready_refresh());
}

#[test]
fn current182_initialization_new_connection_waits_for_old_cleanup_and_keeps_its_own_marker() {
    let mut state = State::default();
    let old = state
        .initialize_connection(input(Some("old-route/profile"), 3))
        .unwrap();
    assert!(
        state
            .initialize_connection(input(Some("new-route/profile"), 4))
            .is_none()
    );
    assert!(!old.is_valid());
    assert!(state.owns_completion(&old));
    let successor = state.initialization_queued.as_ref().unwrap().clone();
    assert!(successor.is_valid());
    assert_eq!(successor.scope, scope(4));
    assert_eq!(
        successor.connection_key.as_deref(),
        Some("new-route/profile")
    );
    assert!(!state.take_ready_refresh());

    state.finish_initialization(&old, false);
    assert_eq!(
        state.initialization_key.as_deref(),
        Some("new-route/profile")
    );
    assert!(state.release(&old));
    let admitted = state.take_queued(Some(scope(4))).unwrap();
    assert_eq!(admitted.sequence(), successor.sequence());
    assert!(admitted.is_valid());
    assert!(state.owns_completion(&admitted));
    assert!(!state.release(&old));
    assert!(!state.take_ready_refresh());

    state.finish_initialization(&admitted, true);
    assert!(state.release(&admitted));
    assert!(state.take_ready_refresh());
}

#[test]
fn current182_initialization_same_path_after_disconnect_starts_a_new_generation() {
    let mut state = State::default();
    let old = state
        .initialize_connection(input(Some("same-path/profile"), 3))
        .unwrap();
    assert!(state.initialize_connection(input(None, 4)).is_none());
    assert!(!old.is_valid());
    assert!(state.connection_key.is_none());
    assert!(state.initialization_key.is_none());
    assert!(!state.take_ready_refresh());
    assert!(
        state
            .initialize_connection(input(Some("same-path/profile"), 5))
            .is_none()
    );
    state.finish_initialization(&old, false);
    assert!(state.release(&old));
    let successor = state.take_queued(Some(scope(5))).unwrap();
    assert_ne!(old.sequence(), successor.sequence());
    assert_eq!(successor.scope, scope(5));
    assert!(successor.is_valid());
    assert_eq!(
        state.initialization_key.as_deref(),
        Some("same-path/profile")
    );
}

#[test]
fn current182_initialization_failure_allows_a_later_same_connection_retry() {
    let mut state = State::default();
    let failed = state
        .initialize_connection(input(Some("first-route/profile"), 3))
        .unwrap();
    state.finish_initialization(&failed, false);
    assert!(state.initialization_key.is_none());
    assert!(state.release(&failed));
    assert!(state.take_ready_refresh());
    assert!(state.pending.is_none());
    let retried = state
        .initialize_connection(input(Some("first-route/profile"), 3))
        .unwrap();
    assert_ne!(failed.sequence(), retried.sequence());
    assert!(retried.is_valid());
    assert!(!state.take_ready_refresh());
}

#[test]
fn current182_initialization_does_not_replace_newer_user_edits_waiting_for_cleanup() {
    let mut state = State::default();
    let old = state
        .initialize_connection(input(Some("old-route/profile"), 3))
        .unwrap();
    state.initialize_connection(input(Some("new-route/profile"), 4));
    let initialize = state.initialization_queued.as_ref().unwrap().clone();
    let mut edit = initialize.clone();
    state.sequence += 1;
    edit.sequence = state.sequence;
    edit.profile = Some(json!({"dpiStages":{"active":2}}));
    edit.change.as_mut().unwrap().initialize = false;
    edit.valid = Arc::new(AtomicBool::new(true));
    assert!(state.enqueue(edit.clone()).is_none());
    assert!(initialize.is_valid());
    assert!(edit.is_valid());
    state.finish_initialization(&old, false);
    assert!(state.release(&old));

    let first = state.take_queued(Some(scope(4))).unwrap();
    assert_eq!(first.sequence(), initialize.sequence());
    assert!(state.queued.as_ref().unwrap().is_valid());
    state.finish_initialization(&first, true);
    assert!(state.release(&first));
    let second = state.take_queued(Some(scope(4))).unwrap();
    assert_eq!(second.sequence(), edit.sequence());
    assert!(second.is_valid());
    assert!(!state.take_ready_refresh());
    assert!(state.release(&second));
    assert!(state.take_ready_refresh());
}

#[test]
fn current182_initialization_macro_exclusion_cancels_queued_initializer_and_can_resume() {
    let mut state = State::default();
    let old = state
        .initialize_connection(input(Some("old-route/profile"), 3))
        .unwrap();
    state.initialize_connection(input(Some("new-route/profile"), 4));
    let queued = state.initialization_queued.as_ref().unwrap().clone();
    let mut excluded = input(Some("new-route/profile"), 4);
    excluded.macro_recording = true;
    assert!(state.initialize_connection(excluded).is_none());
    assert!(!queued.is_valid());
    assert!(state.initialization_queued.is_none());
    assert!(state.initialization_key.is_none());
    state.initialize_connection(input(Some("new-route/profile"), 4));
    let resumed = state.initialization_queued.as_ref().unwrap().clone();
    assert!(resumed.is_valid());
    assert_ne!(queued.sequence(), resumed.sequence());
    assert!(state.release(&old));
    assert_eq!(
        state.take_queued(Some(scope(4))).unwrap().sequence(),
        resumed.sequence()
    );
}

#[test]
fn current182_initialization_drop_cancels_the_deferred_source_initializer() {
    let mut state = State::default();
    let old = state
        .initialize_connection(input(Some("old-route/profile"), 3))
        .unwrap();
    state.initialize_connection(input(Some("new-route/profile"), 4));
    let queued = state.initialization_queued.as_ref().unwrap().clone();
    drop(state);
    assert!(!old.is_valid());
    assert!(!queued.is_valid());
}

#[test]
fn current182_initialization_absent_source_profile_never_creates_a_default_setter() {
    let mut state = State::default();
    let mut missing = input(Some("first-route/profile"), 3);
    missing.stages = None;
    assert!(state.initialize_connection(missing).is_none());
    assert!(state.pending.is_none());
    assert!(state.initialization_key.is_none());
    assert!(state.take_ready_refresh());
    let mut missing_guid = input(Some("first-route/other-profile"), 4);
    missing_guid.profile_guid = "";
    assert!(state.initialize_connection(missing_guid).is_none());
    assert!(state.pending.is_none());
    assert!(state.initialization_key.is_none());
    assert!(state.take_ready_refresh());
}

#[test]
fn current182_initialization_identity_read_is_deduplicated_without_queuing_a_premature_get() {
    let mut state = State::default();
    let identity = state.request_identity(scope(3)).unwrap();
    assert!(identity.is_valid());
    assert!(identity.change().unwrap().is_identity_read());
    assert!(identity.change().unwrap().stages.is_none());
    for _ in 0..3 {
        assert!(state.request_identity(scope(3)).is_none());
        assert!(state.initialization_queued.is_none());
        assert!(state.owns_completion(&identity));
    }
    assert!(state.release(&identity));
    assert!(!state.take_ready_refresh());
    let initialize = state
        .initialize_connection(input(Some("serial-selected-profile"), 3))
        .unwrap();
    assert!(initialize.is_valid());
    assert!(!initialize.change().unwrap().is_identity_read());
    assert!(!state.take_ready_refresh());
}

#[test]
fn current_profile_observation_starts_with_identity_and_never_initializes_imported_settings() {
    for product_id in [182, 190] {
        let mut state = State::default();
        state.local_profile_known = true;
        state.local_profile_error = Some("previous source read".into());
        let identity = state.request_identity(scope(3)).unwrap();
        assert!(!state.local_profile_known);
        assert!(state.local_profile_error.is_none());
        razer_device::mouse_current_dpi::validate_change_for(
            product_id,
            identity.change().unwrap(),
        )
        .unwrap();
        assert!(!state.take_ready_refresh());
        assert!(state.initialization_key.is_none());
        assert!(state.release(&identity));
        // The separately verified source-profile reader supplies local flags;
        // establishing their mapping does not enqueue a settings mutation.
        let mut profile = source_profile();
        profile.enabled = false;
        assert!(Selection::capture(product_id, &profile).is_some());
        assert!(state.install_connection(
            Some("actual-serial/selected-source-guid".into()),
            Some(scope(3))
        ));
        state.request_refresh();
        assert!(state.pending.is_none() && state.initialization_queued.is_none());
        assert!(state.initialization_key.is_none());
        assert!(state.take_ready_refresh());
        state.cancel();
        assert!(state.connection_key.is_none());
        let reconnected = state.request_identity(scope(4)).unwrap();
        assert!(reconnected.change().unwrap().is_identity_read());
        assert!(!state.take_ready_refresh());
    }
}

#[test]
fn current182_initialization_reconnected_identity_waits_for_cleanup_then_profile_init_precedes_get()
{
    let mut state = State::default();
    let old = state
        .initialize_connection(input(Some("old-route/profile"), 3))
        .unwrap();
    state.cancel();
    assert!(state.request_identity(scope(4)).is_none());
    let identity = state.initialization_queued.as_ref().unwrap().clone();
    assert!(!old.is_valid());
    assert!(identity.is_valid());
    assert!(identity.change().unwrap().is_identity_read());
    assert!(!state.take_ready_refresh());
    assert!(state.release(&old));
    let admitted = state.take_queued(Some(scope(4))).unwrap();
    assert_eq!(admitted.sequence(), identity.sequence());
    assert!(admitted.is_valid());
    assert!(state.release(&admitted));
    assert!(!state.take_ready_refresh());

    let initialize = state
        .initialize_connection(input(Some("new-route/serial-profile"), 4))
        .unwrap();
    assert!(initialize.is_valid());
    assert!(!initialize.change().unwrap().is_identity_read());
    assert!(!state.take_ready_refresh());
    state.finish_initialization(&initialize, true);
    assert!(state.release(&initialize));
    assert!(state.take_ready_refresh());
}

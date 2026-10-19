use super::*;
use gpui_kit::EntityId;
use razer_device::mouse_dpi_stages::PackedDpiStage;

fn scope(owner: u64, connection: u64) -> MousePollingScope {
    MousePollingScope::new(EntityId::from(owner), 1, connection)
}

fn draft() -> DpiStagesDraft {
    DpiStagesDraft {
        enabled: true,
        active_stage: 3,
        stages: [400, 800, 1600, 3200, 6400]
            .map(|dpi| DpiStage {
                x: dpi,
                y: dpi,
                visible: true,
                independent: false,
            })
            .into(),
    }
}

fn reading(dpi: u16) -> DpiStagesReading {
    DpiStagesReading {
        profile_id: 1,
        active_stage: 1,
        number_of_active_stages: 1,
        records: vec![PackedDpiStage {
            index: 1,
            x: dpi,
            y: dpi + 100,
            z: 0,
        }],
        response_bytes: 10,
    }
}

#[::core::prelude::v1::test]
fn dpi_stage_observation_uses_actual_table_without_mutating_local_draft() {
    let mut state = State::default();
    let scope = scope(1, 1);
    state.retain_scope(Some(scope));
    let profile = draft();
    let revision = state.begin_read(scope).unwrap();
    let actual = reading(2300);
    assert_eq!(
        state.finish_read(scope, revision, Some(scope), false, Some(actual.clone())),
        Some(true)
    );
    assert_eq!(state.observed, Some((scope, actual)));
    assert_eq!(profile.stages[2].x, 1600);
    assert!(profile.enabled);
    assert!(!profile.stages[2].independent);
}

#[::core::prelude::v1::test]
fn dpi_stage_observation_failed_read_clears_previous_table_and_keeps_queued_draft() {
    let mut state = State::default();
    let scope = scope(1, 1);
    state.retain_scope(Some(scope));
    let first = state.begin_read(scope).unwrap();
    state.finish_read(scope, first, Some(scope), false, Some(reading(2300)));
    let next = state.begin_read(scope).unwrap();
    let draft = draft();
    state.queued = Some(Intent {
        scope,
        revision: next,
        draft: draft.clone(),
        valid_scope: Arc::new(AtomicBool::new(true)),
    });
    assert_eq!(
        state.finish_read(scope, next, Some(scope), false, None),
        Some(true)
    );
    assert!(state.observed.is_none());
    assert_eq!(state.queued.as_ref().unwrap().draft, draft);
    assert!(state.reading.is_none());
    assert!(state.reading_valid_scope.is_none());
}

#[::core::prelude::v1::test]
fn dpi_stage_observation_failed_write_clears_table_without_rolling_back_user_draft() {
    let mut state = State::default();
    let scope = scope(1, 1);
    state.retain_scope(Some(scope));
    state.observe_table(scope, Some(reading(2300)));
    let draft = draft();
    state.pending = Some(Intent {
        scope,
        revision: state.revision,
        draft: draft.clone(),
        valid_scope: Arc::new(AtomicBool::new(true)),
    });
    assert_eq!(
        state.finish_write(
            scope,
            state.revision,
            &draft,
            Some(scope),
            Some(&draft),
            None
        ),
        Some(true)
    );
    assert!(state.observed.is_none());
    assert!(state.pending.is_none());
    assert_eq!(draft.stages[2].x, 1600);
    assert_eq!(draft.active_stage, 3);
}

#[::core::prelude::v1::test]
fn dpi_stage_observation_edit_invalidates_read_without_overwriting_draft() {
    let mut state = State::default();
    let scope = scope(1, 1);
    state.retain_scope(Some(scope));
    let revision = state.begin_read(scope).unwrap();
    let mut draft = draft();
    state.input_changed();
    draft.stages[2].x = 2700;
    assert_eq!(
        state.finish_read(scope, revision, Some(scope), false, Some(reading(2300))),
        Some(false)
    );
    assert!(state.editing);
    assert!(state.observed.is_none());
    assert_eq!(draft.stages[2].x, 2700);
}

#[::core::prelude::v1::test]
fn dpi_stage_observation_scope_replacement_holds_worker_until_cleanup_then_rejects_late_result() {
    for next_scope in [scope(1, 2), scope(2, 1)] {
        let mut state = State::default();
        let previous = scope(1, 1);
        state.retain_scope(Some(previous));
        let revision = state.begin_read(previous).unwrap();
        let guard = state.reading_valid_scope.clone().unwrap();
        state.retain_scope(Some(next_scope));
        assert!(!guard.load(Ordering::Acquire));
        assert!(state.begin_read(next_scope).is_none());
        assert_eq!(
            state.finish_read(
                previous,
                revision,
                Some(next_scope),
                false,
                Some(reading(2300))
            ),
            Some(false)
        );
        assert!(state.observed.is_none());
        let next_revision = state.begin_read(next_scope).unwrap();
        assert_eq!(
            state.finish_read(
                next_scope,
                next_revision,
                Some(next_scope),
                false,
                Some(reading(3500))
            ),
            Some(true)
        );
        assert_eq!(
            state.finish_read(previous, revision, Some(next_scope), false, None),
            None
        );
        assert_eq!(state.observed.as_ref().unwrap().1.records[0].x, 3500);
    }
}

#[::core::prelude::v1::test]
fn dpi_stage_observation_old_write_cannot_release_successor_or_clear_its_observation() {
    let mut state = State::default();
    let scope = scope(1, 1);
    state.retain_scope(Some(scope));
    let old_revision = state.revision;
    state.input_changed();
    state.editing = false;
    let draft = draft();
    state.pending = Some(Intent {
        scope,
        revision: state.revision,
        draft: draft.clone(),
        valid_scope: Arc::new(AtomicBool::new(true)),
    });
    state.observe_table(scope, Some(reading(3500)));
    assert_eq!(
        state.finish_write(scope, old_revision, &draft, Some(scope), Some(&draft), None),
        None
    );
    assert!(state.pending.is_some());
    assert_eq!(state.observed.as_ref().unwrap().1.records[0].x, 3500);
}

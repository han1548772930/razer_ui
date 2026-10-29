use super::super::mouse_polling::MousePollingScope;
use super::*;
use anyhow::{Context as _, ensure};
use gpui_kit::component::Sizable;
use razer_device::mouse_current_dpi::{DpiChange, DpiPair, DpiReport};
use razer_device::mouse_dpi_stages::{DpiStage, DpiStagesDraft};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[path = "mouse_current_dpi_observation.rs"]
mod observation;
use super::mapping_callbacks_current::{Connection, TemporaryRestoration};
use observation::{Observation, RowUpdate, Selection};

#[derive(Clone)]
pub struct MouseCurrentDpiRequested {
    scope: MousePollingScope,
    sequence: u64,
    profile: Option<Value>,
    profile_save: Arc<AtomicBool>,
    change: Option<DpiChange>,
    read: bool,
    cleanup: bool,
    drain_on_exit: bool,
    restoration_ticket: u64,
    connection_key: Option<String>,
    edit_revision: u64,
    selection: Option<Selection>,
    valid: Arc<AtomicBool>,
}

/// Profile-save identities present when a Matcher stage save entered the
/// serialized writer. Later ordinary edits own different permits.
pub struct MouseCurrentDpiProfileSaveBoundary {
    scope: MousePollingScope,
    connection_key: String,
    permits: Vec<Arc<AtomicBool>>,
}

#[cfg(test)]
mod tests {
    use super::{
        MouseCurrentDpiRequested, MousePollingObservation, MousePollingScope,
        MouseProductWorkspace, PollingConnection, State,
    };
    use gpui_kit::{AppContext, EntityId, TestAppContext, px, size};
    use std::sync::{Arc, atomic::AtomicBool};
    fn configured_stages() -> super::DpiStagesDraft {
        super::DpiStagesDraft {
            enabled: true,
            active_stage: 3,
            stages: [400, 800, 1600, 3200, 6400]
                .into_iter()
                .map(|dpi| super::DpiStage {
                    x: dpi,
                    y: dpi,
                    visible: true,
                    independent: false,
                })
                .collect(),
        }
    }

    #[::core::prelude::v1::test]
    fn current190_source_separates_stage_table_edits_from_transient_stage_selection() {
        use super::{Edit, Tasks};
        for edit in [
            Edit::Stages,
            Edit::Profile,
            Edit::AxisMode { reset_y: true },
        ] {
            assert_eq!(
                edit.tasks(190),
                Tasks {
                    stages: true,
                    active: false
                }
            );
        }
        assert_eq!(
            Edit::SelectStage.tasks(190),
            Tasks {
                stages: true,
                active: true
            }
        );
        assert_eq!(
            Edit::AxisMode { reset_y: false }.tasks(190),
            Tasks {
                stages: false,
                active: false
            }
        );
    }

    #[::core::prelude::v1::test]
    fn current182_stage_actions_do_not_issue_the_separate_otfs_level_task() {
        use super::{Edit, Tasks};
        for edit in [
            Edit::Stages,
            Edit::SelectStage,
            Edit::Profile,
            Edit::AxisMode { reset_y: true },
        ] {
            assert_eq!(
                edit.tasks(182),
                Tasks {
                    stages: true,
                    active: false
                }
            );
        }
    }

    // Compile-checked GPUI owner integration. No application/window acceptance
    // is inferred from this test; execution remains outside static verification.
    #[gpui_kit::test]
    fn source_mouse182_number_commit_emits_only_stage_task_and_scope_change_cancels_it(
        cx: &mut TestAppContext,
    ) {
        use gpui_kit::component::Root;
        cx.update(|cx| gpui_kit::init(cx));
        let mut page = None;
        let handle = cx.open_window(size(px(1400.), px(1100.)), |window, cx| {
            let owner = cx.new(|cx| MouseProductWorkspace::new(182, window, cx));
            page = Some(owner.clone());
            Root::new(owner, window, cx)
        });
        let page = page.unwrap();
        let request = cx
            .update_window(handle.into(), |_, window, cx| {
                page.update(cx, |owner, cx| {
                    owner.restore_current_dpi_local_profile(Ok(Some(configured_stages())), cx);
                    let previous_active = owner.draft.get("dpi").cloned();
                    owner.write_number("/dpiStages/stages/2/x", 2000., window, cx);
                    let request = owner
                        .current_dpi
                        .pending
                        .as_ref()
                        .expect("actual Source DPI owner emitted edit")
                        .clone();
                    let change = request.change().unwrap();
                    assert_eq!(
                        change.stages.as_ref().unwrap().stages()[2]
                            .x()
                            .unwrap()
                            .value(),
                        2000.
                    );
                    assert_eq!(change.stages.as_ref().unwrap().stages().len(), 5);
                    assert!(change.active.is_none());
                    assert_eq!(owner.draft.get("dpi").cloned(), previous_active);
                    assert_eq!(
                        request.profile().unwrap()["dpiStages"]["stages"][2]["x"],
                        2000
                    );
                    assert!(owner.current_dpi_request_matches(&request, cx));
                    owner.observe_mouse_polling(
                        request.scope(),
                        MousePollingObservation::Connection(Some(PollingConnection::Dongle)),
                        cx,
                    );
                    assert!(!request.is_valid());
                    assert!(!owner.current_dpi_request_matches(&request, cx));
                    owner.finish_current_dpi(&request, None, cx);
                    assert!(owner.current_dpi.pending.is_none());
                    assert_eq!(owner.draft["dpiStages"]["stages"][2]["x"], 2000);
                    request
                })
            })
            .unwrap();
        assert!(!request.is_valid());
    }

    #[::core::prelude::v1::test]
    fn source_dpi_owner_drop_invalidates_in_flight_and_queued_guards() {
        let request = MouseCurrentDpiRequested {
            scope: MousePollingScope::new(EntityId::from(1), 2, 3),
            sequence: 1,
            profile: None,
            profile_save: Arc::new(AtomicBool::new(true)),
            change: None,
            read: true,
            cleanup: false,
            drain_on_exit: false,
            restoration_ticket: 0,
            connection_key: None,
            edit_revision: 0,
            selection: None,
            valid: Arc::new(AtomicBool::new(true)),
        };
        let mut queued = request.clone();
        queued.sequence = 2;
        queued.valid = Arc::new(AtomicBool::new(true));
        let mut state = State::default();
        state.sequence = 2;
        state.pending = Some(request.clone());
        state.queued = Some(queued.clone());
        drop(state);
        assert!(!request.is_valid());
        assert!(!queued.is_valid());
    }

    fn read(sequence: u64, connection: &str) -> MouseCurrentDpiRequested {
        MouseCurrentDpiRequested {
            scope: MousePollingScope::new(EntityId::from(1), 2, 3),
            sequence,
            profile: None,
            profile_save: Arc::new(AtomicBool::new(true)),
            change: None,
            read: true,
            cleanup: false,
            drain_on_exit: false,
            restoration_ticket: 0,
            connection_key: Some(connection.into()),
            edit_revision: 0,
            selection: None,
            valid: Arc::new(AtomicBool::new(true)),
        }
    }

    fn matcher_stage_request(sequence: u64, connection: &str) -> MouseCurrentDpiRequested {
        let mut request = read(sequence, connection);
        request.read = false;
        request.drain_on_exit = true;
        request.change = Some(super::DpiChange {
            stages: Some(configured_stages().into()),
            active: None,
            profile_guid: None,
            initialize: false,
        });
        request
    }

    fn ordinary_stage_request(sequence: u64, connection: &str) -> MouseCurrentDpiRequested {
        let mut request = matcher_stage_request(sequence, connection);
        request.drain_on_exit = false;
        request.profile = Some(serde_json::json!({
            "guid":"current",
            "dpiStages":{"enable":true,"active":1,"stages":[{
                "x":1200,"y":1200,"visible":true,"independent":false
            }]}
        }));
        request.change.as_mut().unwrap().stages = Some(
            razer_device::mouse_current_dpi::SourceStages::from_source_file(
                &request.profile.as_ref().unwrap()["dpiStages"],
            )
            .unwrap(),
        );
        request
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_restores_held_clutch_without_an_existing_request() {
        let mut owner = State::default();
        let identity = read(1, "receiver/profile");
        owner.install_connection(identity.connection_key.clone(), Some(identity.scope));
        let stages = configured_stages();
        let request = owner
            .prepare_exit(Some(identity.scope), Some(stages.clone()), None)
            .unwrap()
            .expect("held clutch requires the complete profile stage task");
        assert_eq!(owner.connection_key, identity.connection_key);
        assert_eq!(owner.connection_scope, Some(identity.scope));
        assert_eq!(request.connection_key, identity.connection_key);
        assert!(request.is_cleanup());
        assert!(request.is_valid());
        assert!(request.profile().is_none());
        let change = request.change().unwrap();
        assert_eq!(change.stages.as_ref(), Some(&(&stages).into()));
        assert!(change.active.is_none());
        assert!(!change.initialize);
        assert!(owner.owns_completion(&request));
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_keeps_worker_barrier_and_drains_latest_durable_profile() {
        let mut owner = State::default();
        let old = read(1, "receiver/profile");
        owner.install_connection(old.connection_key.clone(), Some(old.scope));
        owner.sequence = 3;
        assert!(owner.enqueue(old.clone()).is_some());
        let mut saved = read(2, "receiver/profile");
        saved.profile = Some(serde_json::json!({"guid":"current", "dpi":1800}));
        assert!(owner.enqueue(saved.clone()).is_none());
        assert!(
            owner
                .prepare_exit(Some(old.scope), Some(configured_stages()), None)
                .unwrap()
                .is_none()
        );
        assert!(!old.is_valid());
        assert!(!saved.is_valid());
        assert!(owner.owns_completion(&old));
        assert!(owner.take_queued(Some(old.scope)).is_none());
        assert!(owner.release(&old));
        let restore = owner
            .take_queued(Some(old.scope))
            .expect("worker cleanup permits restoration");
        assert_eq!(restore.sequence, 4);
        assert_eq!(restore.profile(), saved.profile());
        assert_eq!(restore.connection_key, old.connection_key);
        assert!(restore.is_cleanup());
        assert!(restore.is_valid());
        assert!(!owner.release(&old));
        assert!(owner.owns_completion(&restore));
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_cannot_restore_through_another_connection_scope() {
        let mut owner = State::default();
        let old = read(1, "receiver/profile");
        owner.install_connection(old.connection_key.clone(), Some(old.scope));
        assert!(owner.enqueue(old.clone()).is_some());
        let replacement = MousePollingScope::new(EntityId::from(1), 2, 4);
        assert!(
            owner
                .prepare_exit(Some(replacement), Some(configured_stages()), None)
                .is_err()
        );
        assert!(owner.owns_completion(&old));
        assert!(!old.is_valid());
        assert!(owner.queued.is_none());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_temporary_restore_requires_verified_current_profile_task() {
        let mut owner = State::default();
        let mut request = read(1, "receiver/profile");
        owner.install_connection(request.connection_key.clone(), Some(request.scope));
        owner.temporary_restoration.require();
        request.restoration_ticket = owner.temporary_restoration.ticket();
        request.change = Some(super::DpiChange {
            stages: Some(configured_stages().into()),
            active: None,
            profile_guid: None,
            initialize: false,
        });
        owner.enqueue(request.clone());
        owner.confirm_temporary_restore(&request, false);
        assert!(owner.temporary_restoration.required());
        owner.cancel();
        owner.confirm_temporary_restore(&request, true);
        assert!(
            owner.temporary_restoration.required(),
            "canceled completion cannot confirm restoration"
        );
        owner.release(&request);
        owner.install_connection(request.connection_key.clone(), Some(request.scope));
        request.valid = Arc::new(AtomicBool::new(true));
        request.sequence = 2;
        owner.enqueue(request.clone());
        owner.confirm_temporary_restore(&request, true);
        assert!(!owner.temporary_restoration.required());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_temporary_restore_old_profile_ack_cannot_clear_newer_queued_clutch() {
        let mut owner = State::default();
        let mut old = read(1, "receiver/profile");
        owner.install_connection(old.connection_key.clone(), Some(old.scope));
        old.change = Some(super::DpiChange {
            stages: Some(configured_stages().into()),
            active: None,
            profile_guid: None,
            initialize: false,
        });
        old.restoration_ticket = owner.temporary_restoration.ticket();
        owner.enqueue(old.clone());
        owner.temporary_restoration.require();
        owner.confirm_temporary_restore(&old, true);
        assert!(owner.temporary_restoration.required());
        owner.release(&old);
        let mut restore = old;
        restore.sequence = 2;
        restore.valid = Arc::new(AtomicBool::new(true));
        restore.restoration_ticket = owner.temporary_restoration.ticket();
        owner.enqueue(restore.clone());
        owner.confirm_temporary_restore(&restore, true);
        assert!(!owner.temporary_restoration.required());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_temporary_clutch_requires_no_local_stage_cache_and_never_initializes_it() {
        let dpi = super::DpiPair { x: 900, y: 1200 };
        let change = super::receiver_callback_change(Some(dpi), None).unwrap();
        assert_eq!(change.active, Some(dpi));
        assert!(change.stages.is_none());
        assert!(!change.initialize);
        assert!(super::receiver_callback_change(None, None).is_none());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_busy_refresh_survives_queued_profile_edit() {
        let mut owner = State::default();
        let first = read(1, "route/profile");
        assert!(owner.enqueue(first.clone()).is_some());
        let mut edit = read(2, "route/profile");
        edit.read = false;
        edit.profile = Some(serde_json::json!({"dpiStages":{"active":3}}));
        assert!(owner.enqueue(edit.clone()).is_none());
        owner.request_refresh();
        assert!(!owner.take_ready_refresh());
        assert!(owner.release(&first));
        let queued = owner.queued.take().expect("refresh retains the user edit");
        assert_eq!(queued.sequence, 2);
        assert!(owner.enqueue(queued).is_some());
        assert!(!owner.take_ready_refresh());
        assert!(owner.release(&edit));
        assert!(owner.take_ready_refresh());
        assert!(!owner.take_ready_refresh());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_edit_after_get_prevents_profile_projection() {
        let mut owner = State::default();
        owner.connection_key = Some("route/profile".into());
        let request = read(1, "route/profile");
        owner.enqueue(request.clone());
        assert!(owner.projection_current(&request));
        owner.record_edit();
        assert!(!owner.projection_current(&request));
        assert!(owner.owns_completion(&request));
        assert!(owner.release(&request));
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_write_readback_updates_rows_without_becoming_success() {
        let mut owner = State::default();
        let mut request = read(1, "route/profile");
        request.read = false;
        request.change = Some(super::DpiChange {
            stages: None,
            active: Some(super::DpiPair { x: 2200, y: 2200 }),
            profile_guid: None,
            initialize: false,
        });
        request.selection = Some(
            super::Selection::capture(
                190,
                &super::DpiStagesDraft {
                    enabled: false,
                    active_stage: 3,
                    stages: [400, 800, 1600, 3200, 6400]
                        .map(|value| super::DpiStage {
                            x: value,
                            y: value,
                            visible: true,
                            independent: false,
                        })
                        .into(),
                },
            )
            .unwrap(),
        );
        owner.connection_key = request.connection_key.clone();
        owner.row_values[0] = Some(super::DpiPair { x: 500, y: 500 });
        owner.enqueue(request.clone());
        let report = serde_json::from_value(serde_json::json!({
            "requested":request.change, "stages":null, "active":{"x":2300,"y":2400},
            "acknowledgements":[], "verified":false,
            "write_errors":{"active":"readback differs","stages":null}
        }))
        .unwrap();
        assert!(owner.accept_report(&request, &report).is_some());
        assert_eq!(
            owner.row_values[2],
            Some(super::DpiPair { x: 2300, y: 2400 })
        );
        assert_eq!(owner.row_values[0], Some(super::DpiPair { x: 500, y: 500 }));
        assert!(
            owner
                .read_error
                .as_ref()
                .unwrap()
                .contains("readback differs")
        );
        owner.record_edit();
        assert!(
            owner
                .observe_rows(&request, [super::RowUpdate::Unobserved; 5])
                .is_none()
        );
        assert!(owner.row_values[2].is_some());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_raw_stage_failure_keeps_number_identity_actual_zero_and_draft_guard() {
        use razer_device::mouse_current_dpi::{SourceNumber, SourceStage, SourceStages};
        use serde_json::json;

        let raw = SourceNumber::new(f64::from_bits(0x7ff8_0000_0000_0042));
        let stages = SourceStages::new(
            true,
            1,
            vec![SourceStage::new(
                Some(raw),
                Some(raw),
                Some(true),
                Some(false),
            )],
        );
        let mut request = read(1, "route/profile");
        request.read = false;
        request.profile = Some(json!({
            "dpiStages":{"active":1,"stages":[{"x":null,"y":null}]}
        }));
        request.change = Some(super::DpiChange {
            stages: Some(stages.clone()),
            active: None,
            profile_guid: None,
            initialize: false,
        });
        request.selection = Some(super::Selection::capture_source(190, &stages).unwrap());
        let expected_change = request.change.clone();
        let expected_profile = request.profile.clone();

        let mut owner = State::default();
        owner.install_connection(request.connection_key.clone(), Some(request.scope));
        assert!(owner.enqueue(request.clone()).is_some());
        let report: super::DpiReport = serde_json::from_value(json!({
            "requested":expected_change,
            "stages":{"profile_id":1,"active_stage":1,"number_of_active_stages":1,
                "records":[{"index":1,"x":0,"y":0,"z":0}],"response_bytes":10},
            "active":null,"acknowledgements":[],"verified":false,
            "read_errors":{"stages":null,"active":null},
            "write_errors":{"stages":"strict getter differs","active":null}
        }))
        .unwrap();
        assert!(!report.verified);
        assert_eq!(report.requested.as_ref(), expected_change.as_ref());
        assert_eq!(report.stages.as_ref().unwrap().records[0].x, 0);
        assert!(owner.accept_report(&request, &report).is_some());
        assert_eq!(owner.row_values[0], Some(super::DpiPair { x: 0, y: 0 }));
        assert!(
            owner
                .read_error
                .as_deref()
                .is_some_and(|error| error.contains("strict getter differs"))
        );
        let pending = owner.pending.as_ref().unwrap();
        let pending_stage = &pending
            .change
            .as_ref()
            .unwrap()
            .stages
            .as_ref()
            .unwrap()
            .stages()[0];
        assert_eq!(
            pending_stage.x().unwrap().value().to_bits(),
            raw.value().to_bits()
        );
        assert_eq!(
            pending_stage.y().unwrap().value().to_bits(),
            raw.value().to_bits()
        );
        assert_eq!(pending.change.as_ref(), expected_change.as_ref());
        assert_eq!(pending.profile(), expected_profile.as_ref());
        assert!(request.is_valid());
        assert!(owner.owns_completion(&request));
    }

    #[::core::prelude::v1::test]
    fn current_dpi_adjacent_edit_revision_keeps_new_draft_separate_from_old_actual_report() {
        use serde_json::json;

        let mut owner = State::default();
        let mut first = read(1, "route/profile");
        first.selection = Some(super::Selection::capture(190, &configured_stages()).unwrap());
        owner.install_connection(first.connection_key.clone(), Some(first.scope));
        assert!(owner.enqueue(first.clone()).is_some());
        let report = |x, y| {
            serde_json::from_value(json!({
                "requested":null,"stages":null,"active":{"x":x,"y":y},
                "acknowledgements":[],"verified":false,
                "write_errors":{"stages":null,"active":"actual differs"}
            }))
            .unwrap()
        };
        assert!(owner.accept_report(&first, &report(2100, 2200)).is_some());
        let actual = owner.row_values;

        owner.record_edit();
        let mut next = read(2, "route/profile");
        next.read = false;
        next.edit_revision = owner.edit_revision;
        next.selection = first.selection.clone();
        next.profile = Some(json!({"dpiStages":{"active":3,"stages":[
            {"x":400,"y":400},{"x":800,"y":800},{"x":777,"y":778},
            {"x":3200,"y":3200},{"x":6400,"y":6400}
        ]}}));
        next.change = Some(super::DpiChange {
            stages: None,
            active: Some(super::DpiPair { x: 777, y: 778 }),
            profile_guid: None,
            initialize: false,
        });
        assert_eq!(next.edit_revision, first.edit_revision + 1);
        let draft = next.profile.clone();
        let change = next.change.clone();
        assert!(owner.enqueue(next.clone()).is_none());
        assert!(owner.accept_report(&first, &report(2500, 2600)).is_none());
        assert_eq!(
            owner.observed.current,
            Some(super::DpiPair { x: 2500, y: 2600 })
        );
        assert_eq!(owner.row_values, actual);
        assert_eq!(owner.queued.as_ref().unwrap().profile, draft);
        assert_eq!(owner.queued.as_ref().unwrap().change, change);
        assert!(owner.owns_completion(&first));
        assert!(next.is_valid());

        assert!(owner.release(&first));
        let current = owner.take_queued(Some(next.scope)).unwrap();
        assert_eq!(current.edit_revision, next.edit_revision);
        assert!(owner.accept_report(&current, &report(790, 791)).is_some());
        assert_eq!(owner.row_values[2], Some(super::DpiPair { x: 790, y: 791 }));
        assert_eq!(current.profile, draft);
        assert_eq!(current.change, change);
        assert!(
            owner
                .read_error
                .as_deref()
                .unwrap()
                .contains("actual differs")
        );
    }

    #[::core::prelude::v1::test]
    fn current_dpi_normal_skippable_keeps_pending_replaces_queued_and_releases_canceled_guard() {
        let mut owner = State::default();
        let pending = read(1, "route/profile");
        let first_queued = read(2, "route/profile");
        let latest_queued = read(3, "route/profile");
        assert!(owner.enqueue(pending.clone()).is_some());
        assert!(owner.enqueue(first_queued.clone()).is_none());
        assert!(owner.enqueue(latest_queued.clone()).is_none());
        assert!(pending.is_valid());
        assert!(latest_queued.is_valid());
        assert!(owner.owns_completion(&pending));
        assert!(!first_queued.is_valid());
        assert!(!owner.owns_completion(&first_queued));
        assert!(!owner.release(&first_queued));
        assert!(
            owner
                .queued
                .as_ref()
                .is_some_and(|request| request.sequence == 3)
        );
        owner.cancel();
        assert!(owner.pending.is_some());
        assert!(!pending.is_valid());
        assert!(!latest_queued.is_valid());
        assert!(owner.queued.is_none());
        assert!(owner.release(&pending));
        assert!(owner.pending.is_none());
        assert!(!owner.release(&pending));
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_drains_admitted_matcher_stages_without_clutch_cleanup() {
        let mut owner = State::default();
        let mut pending = read(1, "route/profile");
        pending.read = false;
        pending.drain_on_exit = true;
        pending.change = Some(super::DpiChange {
            stages: Some(configured_stages().into()),
            active: None,
            profile_guid: None,
            initialize: false,
        });
        let mut queued = pending.clone();
        queued.sequence = 2;
        queued.valid = Arc::new(AtomicBool::new(true));
        let expected_change = queued.change.clone();
        owner.install_connection(pending.connection_key.clone(), Some(pending.scope));
        assert!(!read(3, "route/profile").can_finish_during_exit());
        assert!(owner.enqueue(pending.clone()).is_some());
        assert!(owner.enqueue(queued.clone()).is_none());
        assert!(
            owner
                .prepare_exit(Some(pending.scope), None, None)
                .unwrap()
                .is_none()
        );
        for request in [&pending, &queued] {
            assert!(request.is_valid());
            assert!(request.can_finish_during_exit());
            assert!(!request.is_cleanup());
            assert!(request.profile().is_none());
        }
        assert!(owner.owns_completion(&pending));
        assert!(owner.take_queued(Some(pending.scope)).is_none());
        assert!(owner.release(&pending));
        let next = owner.take_queued(Some(queued.scope)).unwrap();
        assert_eq!(next.sequence, queued.sequence);
        assert!(Arc::ptr_eq(&next.guard(), &queued.guard()));
        assert_eq!(next.change, expected_change);
        assert!(next.is_valid());
        assert!(next.can_finish_during_exit());
        assert!(!next.is_cleanup());
        assert!(!owner.release(&pending));
        assert!(owner.owns_completion(&next));
        assert!(owner.release(&next));
        assert!(!queued.is_valid());
        assert!(owner.pending.is_none());
        assert!(owner.queued.is_none());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_ordinary_then_matcher_keeps_newer_work_without_resaving_old_profile() {
        let mut owner = State::default();
        let ordinary = ordinary_stage_request(1, "route/profile");
        let matcher = matcher_stage_request(2, "route/profile");
        owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
        owner.sequence = 2;
        assert!(owner.enqueue(ordinary.clone()).is_some());
        assert!(owner.enqueue(matcher.clone()).is_none());
        assert!(
            owner
                .prepare_exit(Some(ordinary.scope), None, None)
                .unwrap()
                .is_none()
        );
        assert!(!ordinary.is_valid());
        assert!(matcher.is_valid());
        assert!(owner.release(&ordinary));
        let next = owner.take_queued(Some(matcher.scope)).unwrap();
        assert!(Arc::ptr_eq(&next.guard(), &matcher.guard()));
        assert!(next.profile().is_none());
        assert_eq!(next.change, matcher.change);
        assert!(owner.release(&next));
        assert!(owner.take_queued(Some(matcher.scope)).is_none());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_ordinary_then_matcher_restores_clutch_after_accepted_matcher() {
        let mut owner = State::default();
        let ordinary = ordinary_stage_request(1, "route/profile");
        let matcher = matcher_stage_request(2, "route/profile");
        owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
        owner.sequence = 2;
        assert!(owner.enqueue(ordinary.clone()).is_some());
        assert!(owner.enqueue(matcher.clone()).is_none());
        assert!(
            owner
                .prepare_exit(Some(ordinary.scope), Some(configured_stages()), None)
                .unwrap()
                .is_none()
        );
        assert!(!ordinary.is_valid());
        assert!(matcher.is_valid());
        assert!(owner.release(&ordinary));
        let admitted = owner.take_queued(Some(matcher.scope)).unwrap();
        assert!(Arc::ptr_eq(&admitted.guard(), &matcher.guard()));
        assert!(!admitted.is_cleanup());
        assert!(admitted.profile().is_none());
        assert!(owner.release(&admitted));
        let restore = owner.take_queued(Some(matcher.scope)).unwrap();
        assert!(restore.is_cleanup());
        assert!(restore.profile().is_none());
        assert_eq!(
            restore.change().unwrap().stages.as_ref(),
            Some(&configured_stages().into())
        );
        assert!(owner.release(&restore));
        assert!(owner.take_queued(Some(matcher.scope)).is_none());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_matcher_then_ordinary_retains_the_later_profile_and_clutch_restore() {
        for restore in [None, Some(configured_stages())] {
            let mut owner = State::default();
            let matcher = matcher_stage_request(1, "route/profile");
            let ordinary = ordinary_stage_request(2, "route/profile");
            owner.install_connection(matcher.connection_key.clone(), Some(matcher.scope));
            owner.sequence = 2;
            assert!(owner.enqueue(matcher.clone()).is_some());
            assert!(owner.enqueue(ordinary.clone()).is_none());
            assert!(
                owner
                    .prepare_exit(Some(matcher.scope), restore.clone(), None)
                    .unwrap()
                    .is_none()
            );
            assert!(matcher.is_valid());
            assert!(!ordinary.is_valid());
            assert!(owner.owns_completion(&matcher));
            assert!(owner.release(&matcher));
            let finalizer = owner.take_queued(Some(ordinary.scope)).unwrap();
            assert_eq!(finalizer.profile(), ordinary.profile());
            assert_eq!(finalizer.is_cleanup(), restore.is_some());
            assert_eq!(
                finalizer.change().and_then(|change| change.stages.as_ref()),
                restore.as_ref().map(|stages| stages.into()).as_ref()
            );
            assert!(owner.release(&finalizer));
            assert!(owner.take_queued(Some(ordinary.scope)).is_none());
        }
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_clutch_restore_waits_for_both_admitted_matcher_requests() {
        let mut owner = State::default();
        let first = matcher_stage_request(1, "route/profile");
        let second = matcher_stage_request(2, "route/profile");
        owner.install_connection(first.connection_key.clone(), Some(first.scope));
        owner.sequence = 2;
        assert!(owner.enqueue(first.clone()).is_some());
        assert!(owner.enqueue(second.clone()).is_none());
        assert!(
            owner
                .prepare_exit(Some(first.scope), Some(configured_stages()), None)
                .unwrap()
                .is_none()
        );
        assert!(first.is_valid());
        assert!(second.is_valid());
        assert!(owner.release(&first));
        let admitted = owner.take_queued(Some(second.scope)).unwrap();
        assert!(Arc::ptr_eq(&admitted.guard(), &second.guard()));
        assert!(!admitted.is_cleanup());
        assert!(owner.release(&admitted));
        let restore = owner.take_queued(Some(second.scope)).unwrap();
        assert!(restore.is_cleanup());
        assert!(restore.profile().is_none());
        assert!(owner.release(&restore));
        assert!(owner.take_queued(Some(second.scope)).is_none());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_late_matcher_supersedes_pending_old_profile_without_losing_cleanup() {
        for restore in [None, Some(configured_stages())] {
            let mut owner = State::default();
            let ordinary = ordinary_stage_request(1, "route/profile");
            owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
            owner.sequence = 1;
            assert!(owner.enqueue(ordinary.clone()).is_some());
            let boundary = owner.profile_save_boundary(ordinary.scope).unwrap();
            assert!(
                owner
                    .prepare_exit(Some(ordinary.scope), restore.clone(), None)
                    .unwrap()
                    .is_none()
            );
            owner.matcher_stages_saved(&boundary);
            let matcher = matcher_stage_request(3, "route/profile");
            assert!(owner.enqueue(matcher.clone()).is_none());
            assert!(owner.release(&ordinary));
            let admitted = owner.take_queued(Some(matcher.scope)).unwrap();
            assert!(Arc::ptr_eq(&admitted.guard(), &matcher.guard()));
            assert!(admitted.profile().is_none());
            assert!(owner.release(&admitted));
            let cleanup = owner.take_queued(Some(matcher.scope));
            if let Some(expected) = restore {
                let cleanup = cleanup.expect("clutch restore follows accepted Matcher work");
                assert!(cleanup.is_cleanup());
                assert!(cleanup.profile().is_none());
                assert_eq!(cleanup.change().unwrap().stages, Some(expected.into()));
                assert!(owner.release(&cleanup));
            } else {
                assert!(cleanup.is_none());
            }
            assert!(owner.take_queued(Some(matcher.scope)).is_none());
        }
    }

    #[::core::prelude::v1::test]
    fn current_dpi_exit_cleanup_cancels_with_its_connection_or_owner() {
        for replace_connection in [false, true] {
            let mut owner = State::default();
            let matcher = matcher_stage_request(1, "route/profile");
            owner.install_connection(matcher.connection_key.clone(), Some(matcher.scope));
            owner.sequence = 1;
            assert!(owner.enqueue(matcher.clone()).is_some());
            assert!(
                owner
                    .prepare_exit(Some(matcher.scope), Some(configured_stages()), None)
                    .unwrap()
                    .is_none()
            );
            let cleanup = owner.exit_request.as_ref().unwrap().clone();
            assert!(cleanup.is_valid());
            assert!(cleanup.is_cleanup());
            if replace_connection {
                assert!(
                    owner.install_connection(
                        Some("replacement/profile".into()),
                        Some(matcher.scope)
                    )
                );
                assert!(owner.exit_request.is_none());
                assert!(!cleanup.is_valid());
                assert!(!matcher.is_valid());
            } else {
                drop(owner);
                assert!(!cleanup.is_valid());
                assert!(!matcher.is_valid());
            }
        }
    }

    #[::core::prelude::v1::test]
    fn current_dpi_saved_matcher_skips_promoted_exit_profile_before_late_device_admission() {
        use std::collections::VecDeque;

        for restore in [None, Some(configured_stages())] {
            let mut owner = State::default();
            let ordinary = ordinary_stage_request(1, "route/profile");
            owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
            owner.sequence = 1;
            assert!(owner.enqueue(ordinary.clone()).is_some());
            let boundary = owner.profile_save_boundary(ordinary.scope).unwrap();
            owner
                .prepare_exit(Some(ordinary.scope), restore.clone(), None)
                .unwrap();
            assert!(owner.release(&ordinary));
            let finalizer = owner.take_queued(Some(ordinary.scope)).unwrap();
            let expected_restore = finalizer.change.clone();
            let mut saves = VecDeque::from([finalizer.clone()]);
            assert!(saves.front().unwrap().profile().is_some());

            // The Matcher stage file commits while this finalizer is still in
            // the serialized save queue; StageRequest has not been delivered.
            owner.matcher_stages_saved(&boundary);
            let captured = saves.pop_front().unwrap();
            assert!(captured.profile().is_none());
            assert!(captured.is_valid());
            assert_eq!(captured.change, expected_restore);
            assert_eq!(captured.is_cleanup(), restore.is_some());

            let matcher = matcher_stage_request(3, "route/profile");
            assert!(owner.enqueue(matcher.clone()).is_none());
            assert!(owner.release(&captured));
            let next = owner.take_queued(Some(matcher.scope)).unwrap();
            assert!(Arc::ptr_eq(&next.guard(), &matcher.guard()));
            assert_eq!(next.change, matcher.change);
        }
    }

    #[::core::prelude::v1::test]
    fn current_dpi_saved_matcher_prevents_old_pending_profile_from_reappearing_on_exit() {
        for restore in [None, Some(configured_stages())] {
            let mut owner = State::default();
            let ordinary = ordinary_stage_request(1, "route/profile");
            owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
            owner.sequence = 1;
            assert!(owner.enqueue(ordinary.clone()).is_some());
            let boundary = owner.profile_save_boundary(ordinary.scope).unwrap();
            let requested = ordinary.change.clone();

            // The ordinary device operation is already in flight. A Matcher
            // file commit can precede both exit preparation and StageRequest.
            owner.matcher_stages_saved(&boundary);
            assert!(ordinary.profile().is_none());
            assert!(ordinary.is_valid());
            assert_eq!(ordinary.change, requested);
            assert!(owner.owns_completion(&ordinary));
            owner
                .prepare_exit(Some(ordinary.scope), restore.clone(), None)
                .unwrap();
            assert!(owner.release(&ordinary));
            let cleanup = owner.take_queued(Some(ordinary.scope));
            if let Some(expected) = restore {
                let cleanup = cleanup.unwrap();
                assert!(cleanup.profile().is_none());
                assert!(cleanup.is_cleanup());
                assert_eq!(cleanup.change().unwrap().stages, Some(expected.into()));
            } else {
                assert!(cleanup.is_none());
            }
        }
    }

    #[::core::prelude::v1::test]
    fn current_dpi_saved_matcher_keeps_newer_normal_skippable_profile_edits() {
        for enqueue_during_save in [false, true] {
            let mut owner = State::default();
            let ordinary = ordinary_stage_request(1, "route/profile");
            owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
            owner.sequence = 1;
            assert!(owner.enqueue(ordinary.clone()).is_some());
            let boundary = owner.profile_save_boundary(ordinary.scope).unwrap();
            if !enqueue_during_save {
                owner.matcher_stages_saved(&boundary);
            }

            let mut newer = ordinary_stage_request(2, "route/profile");
            newer.profile.as_mut().unwrap()["dpiStages"]["stages"][0]["x"] =
                serde_json::json!(2400);
            assert!(owner.enqueue(newer.clone()).is_none());
            if enqueue_during_save {
                owner.matcher_stages_saved(&boundary);
            }
            assert!(ordinary.profile().is_none());
            assert!(newer.profile().is_some());
            assert!(newer.is_valid());
            owner
                .prepare_exit(Some(ordinary.scope), None, None)
                .unwrap();
            let matcher = matcher_stage_request(3, "route/profile");
            assert!(owner.enqueue(matcher.clone()).is_none());
            assert!(owner.release(&ordinary));
            let admitted = owner.take_queued(Some(matcher.scope)).unwrap();
            assert!(Arc::ptr_eq(&admitted.guard(), &matcher.guard()));
            assert!(owner.release(&admitted));
            let finalizer = owner.take_queued(Some(ordinary.scope)).unwrap();
            assert_eq!(finalizer.profile(), newer.profile());
            assert_eq!(
                finalizer.profile().unwrap()["dpiStages"]["stages"][0]["x"],
                2400
            );
        }
    }

    #[::core::prelude::v1::test]
    fn current_dpi_saved_matcher_cannot_invalidate_another_scope_or_connection_profile() {
        for scope in [
            MousePollingScope::new(EntityId::from(2), 2, 3),
            MousePollingScope::new(EntityId::from(1), 3, 3),
            MousePollingScope::new(EntityId::from(1), 2, 4),
        ] {
            let mut owner = State::default();
            let ordinary = ordinary_stage_request(1, "route/profile");
            owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
            assert!(owner.enqueue(ordinary.clone()).is_some());
            assert!(owner.profile_save_boundary(scope).is_none());
            let boundary = owner.profile_save_boundary(ordinary.scope).unwrap();
            let mut replacement = ordinary_stage_request(1, "route/profile");
            replacement.scope = scope;
            assert!(owner.install_connection(replacement.connection_key.clone(), Some(scope)));
            assert!(owner.enqueue(replacement.clone()).is_none());
            owner.matcher_stages_saved(&boundary);
            assert!(ordinary.profile().is_some());
            assert!(replacement.profile().is_some());
            assert!(replacement.is_valid());
        }
        let mut owner = State::default();
        let ordinary = ordinary_stage_request(1, "route/profile");
        owner.install_connection(ordinary.connection_key.clone(), Some(ordinary.scope));
        assert!(owner.enqueue(ordinary.clone()).is_some());
        let boundary = owner.profile_save_boundary(ordinary.scope).unwrap();
        let replacement = ordinary_stage_request(1, "replacement/profile");
        assert!(
            owner.install_connection(replacement.connection_key.clone(), Some(replacement.scope))
        );
        assert!(owner.enqueue(replacement.clone()).is_none());
        owner.matcher_stages_saved(&boundary);
        assert!(ordinary.profile().is_some());
        assert!(replacement.profile().is_some());
        assert!(replacement.is_valid());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_drain_marker_does_not_survive_connection_replacement() {
        let mut owner = State::default();
        let mut pending = read(1, "old-route/profile");
        pending.read = false;
        pending.drain_on_exit = true;
        pending.change = Some(super::DpiChange {
            stages: Some(configured_stages().into()),
            active: None,
            profile_guid: None,
            initialize: false,
        });
        let mut queued = pending.clone();
        queued.sequence = 2;
        queued.valid = Arc::new(AtomicBool::new(true));
        owner.install_connection(pending.connection_key.clone(), Some(pending.scope));
        assert!(owner.enqueue(pending.clone()).is_some());
        assert!(owner.enqueue(queued.clone()).is_none());

        let successor = read(1, "new-route/profile");
        assert!(owner.install_connection(successor.connection_key.clone(), Some(successor.scope)));
        assert!(!pending.is_valid());
        assert!(!queued.is_valid());
        assert!(owner.owns_completion(&pending));
        assert!(owner.queued.is_none());
        assert!(owner.enqueue(successor.clone()).is_none());
        assert!(owner.release(&pending));
        let next = owner.take_queued(Some(successor.scope)).unwrap();
        assert!(next.is_valid());
        assert_eq!(next.connection_key, successor.connection_key);
        assert!(!owner.release(&pending));
        assert!(owner.owns_completion(&successor));
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_failed_refresh_removes_the_previous_control_value() {
        let mut owner = State::default();
        let request = read(1, "route/profile");
        owner.connection_key = request.connection_key.clone();
        owner
            .row_values
            .fill(Some(super::DpiPair { x: 1600, y: 1600 }));
        owner.enqueue(request.clone());
        let report = serde_json::from_value(serde_json::json!({
            "requested":null,"stages":null,"active":null,"acknowledgements":[],"verified":false,
            "read_errors":{"active":"get failed","stages":"get failed"}
        }))
        .unwrap();
        assert!(owner.accept_report(&request, &report).is_some());
        assert_eq!(owner.row_values, [None; 5]);
        assert_eq!(owner.observed.errors().count(), 2);
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_old_connection_completion_cannot_release_successor() {
        let mut owner = State::default();
        let old = read(1, "old-route/profile");
        owner.enqueue(old.clone());
        owner.cancel();
        let new = read(1, "new-route/profile");
        owner.enqueue(new.clone());
        assert!(!old.is_valid());
        assert!(owner.release(&old));
        let next = owner.queued.take().unwrap();
        owner.enqueue(next);
        assert!(!owner.release(&old));
        assert!(owner.owns_completion(&new));
        assert!(new.is_valid());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_failure_can_refresh_and_disconnect_clears_values() {
        let mut owner = State::default();
        owner.connection_key = Some("route/profile".into());
        owner.row_values[2] = Some(super::DpiPair { x: 2000, y: 2000 });
        owner.read_error = Some("getter failed".into());
        owner.request_refresh();
        assert!(owner.take_ready_refresh());
        assert!(owner.read_error.is_none());
        assert_eq!(owner.row_values[2].unwrap().x, 2000);
        owner.cancel();
        assert_eq!(owner.row_values, [None; 5]);
        assert!(owner.connection_key.is_none());
        assert!(!owner.refresh_requested);
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_reorder_keeps_observed_identity_with_its_source_row() {
        let mut owner = State::default();
        owner.row_values[2] = Some(super::DpiPair { x: 2000, y: 2000 });
        owner.move_row(2, 4);
        assert_eq!(owner.row_values[2], None);
        assert_eq!(owner.row_values[4].unwrap().x, 2000);
        owner.move_row(4, 1);
        assert_eq!(owner.row_values[4], None);
        assert_eq!(owner.row_values[1].unwrap().x, 2000);
    }

    #[gpui_kit::test]
    fn source_current_dpi_actual_pair_syncs_original_input_and_preserves_uncommitted_text(
        cx: &mut TestAppContext,
    ) {
        use gpui_kit::component::Root;
        use serde_json::json;
        cx.update(|cx| gpui_kit::init(cx));
        let mut page = None;
        let handle = cx.open_window(size(px(1400.), px(1100.)), |window, cx| {
            let owner = cx.new(|cx| MouseProductWorkspace::new(190, window, cx));
            page = Some(owner.clone());
            Root::new(owner, window, cx)
        });
        let page = page.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            page.update(cx, |owner, cx| {
                owner.restore_current_dpi_local_profile(Ok(Some(configured_stages())), cx);
                let path = "/dpiStages/stages/2/x";
                let report = |value| {
                    serde_json::from_value(json!({
                        "requested":null, "stages":null, "active":{"x":value,"y":value},
                        "acknowledgements":[], "verified":false,
                        "read_errors":{"active":null,"stages":"table getter failed"}
                    }))
                    .unwrap()
                };
                owner.request_current_dpi_read(cx);
                let request = owner.current_dpi.pending.clone().unwrap();
                owner.finish_current_dpi_result(&request, Ok(report(2000)), cx);
                assert_eq!(owner.number(path), 2000.);
                owner.sync_current_dpi_controls(window, cx);
                assert_eq!(owner.inputs[path].read(cx).value().to_string(), "2000");
                assert!(owner.boolean("/dpiStages/enable"));
                assert!(!owner.current_dpi_row_available(0));
                owner.dpi_numbers.get_mut(path).unwrap().registered = true;
                owner.inputs[path].update(cx, |input, cx| input.set_value("2300", window, cx));
                owner.request_current_dpi_read(cx);
                let request = owner.current_dpi.pending.clone().unwrap();
                owner.finish_current_dpi_result(&request, Ok(report(2500)), cx);
                owner.sync_current_dpi_controls(window, cx);
                assert_eq!(owner.number(path), 2000.);
                assert_eq!(owner.inputs[path].read(cx).value().to_string(), "2300");
                assert_eq!(owner.current_dpi.observed.current.unwrap().x, 2500);
            });
        })
        .unwrap();
    }
}

#[cfg(test)]
#[path = "mouse_current_dpi_initialization_tests.rs"]
mod initialization_tests;

impl MouseCurrentDpiRequested {
    pub fn scope(&self) -> MousePollingScope {
        self.scope
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn profile(&self) -> Option<&Value> {
        self.profile
            .as_ref()
            .filter(|_| self.profile_save.load(Ordering::Acquire))
    }
    pub fn change(&self) -> Option<&DpiChange> {
        self.change.as_ref()
    }
    pub fn is_read(&self) -> bool {
        self.read
    }
    pub fn is_cleanup(&self) -> bool {
        self.cleanup
    }
    pub fn can_finish_during_exit(&self) -> bool {
        self.cleanup || self.drain_on_exit
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct State {
    sequence: u64,
    pending: Option<MouseCurrentDpiRequested>,
    queued: Option<MouseCurrentDpiRequested>,
    initialization_queued: Option<MouseCurrentDpiRequested>,
    exit_request: Option<MouseCurrentDpiRequested>,
    observed: Observation,
    row_values: [Option<DpiPair>; 5],
    control_sync: BTreeMap<String, u16>,
    connection_key: Option<String>,
    connection_scope: Option<MousePollingScope>,
    edit_revision: u64,
    refresh_requested: bool,
    read_error: Option<String>,
    initialization_key: Option<String>,
    macro_recording: bool,
    local_profile_known: bool,
    local_profile_error: Option<String>,
    temporary_restoration: TemporaryRestoration,
}

#[derive(Clone, Copy)]
pub(super) enum Edit {
    Stages,
    SelectStage,
    AxisMode { reset_y: bool },
    Profile,
}

#[derive(Debug, PartialEq, Eq)]
struct Tasks {
    stages: bool,
    active: bool,
}

struct ProfileInitialization<'a> {
    connection_key: Option<String>,
    scope: Option<MousePollingScope>,
    profile_guid: &'a str,
    stages: Option<DpiStagesDraft>,
    selection: Option<Selection>,
    macro_recording: bool,
}

impl Edit {
    fn tasks(self, product_id: u32) -> Tasks {
        let stages = match self {
            Self::AxisMode { reset_y } => reset_y,
            _ => true,
        };
        // Current190 pA's row edits, enable, visibility and reorder emit only
        // IE/ON_STAGES_CHANGE. pA.setStage also emits IA's transient DPI task.
        // Current182 ZE has no transient task for any stage edit.
        let active =
            product_id != 182 && stages && (product_id != 190 || matches!(self, Self::SelectStage));
        Tasks { stages, active }
    }
}

fn receiver_callback_change(
    temporary: Option<DpiPair>,
    stages: Option<DpiStagesDraft>,
) -> Option<DpiChange> {
    let (stages, active) = match temporary {
        Some(dpi) => (None, Some(dpi)),
        None => (Some(stages?.into()), None),
    };
    Some(DpiChange {
        stages,
        active,
        profile_guid: None,
        initialize: false,
    })
}

impl State {
    pub(super) fn observe_mapping_connection(&mut self, connection: Option<Connection>) {
        self.temporary_restoration.observe_connection(connection);
    }

    fn confirm_temporary_restore(&mut self, request: &MouseCurrentDpiRequested, verified: bool) {
        if verified
            && request.is_valid()
            && self.owns_completion(request)
            && request.connection_key == self.connection_key
            && Some(request.scope) == self.connection_scope
            && request
                .change
                .as_ref()
                .is_some_and(|change| !change.initialize && change.stages.is_some())
        {
            self.temporary_restoration
                .confirm(request.restoration_ticket);
        }
    }

    fn enqueue(&mut self, request: MouseCurrentDpiRequested) -> Option<MouseCurrentDpiRequested> {
        // The durable Matcher save owns profile invalidation. Admission only
        // drops its expired profile-only finalizer; later edits retain theirs.
        if request.drain_on_exit
            && self
                .exit_request
                .as_ref()
                .is_some_and(|exit| !exit.cleanup && exit.profile().is_none())
            && let Some(exit) = self.exit_request.take()
        {
            exit.cancel();
        }
        if self.pending.is_some() {
            if let Some(previous) = self.queued.replace(request) {
                self.finish_initialization(&previous, false);
                previous.cancel();
            }
            None
        } else {
            self.pending = Some(request.clone());
            Some(request)
        }
    }

    fn profile_save_boundary(
        &self,
        scope: MousePollingScope,
    ) -> Option<MouseCurrentDpiProfileSaveBoundary> {
        if self.connection_scope != Some(scope) {
            return None;
        }
        let connection_key = self.connection_key.clone()?;
        let permits = self
            .pending
            .iter()
            .chain(self.queued.iter())
            .chain(self.initialization_queued.iter())
            .chain(self.exit_request.iter())
            .filter(|request| {
                request.scope == scope
                    && request.connection_key.as_ref() == Some(&connection_key)
                    && request.profile.is_some()
            })
            .map(|request| request.profile_save.clone())
            .collect();
        Some(MouseCurrentDpiProfileSaveBoundary {
            scope,
            connection_key,
            permits,
        })
    }

    fn matcher_stages_saved(&mut self, boundary: &MouseCurrentDpiProfileSaveBoundary) {
        if self.connection_scope != Some(boundary.scope)
            || self.connection_key.as_ref() != Some(&boundary.connection_key)
        {
            return;
        }
        for permit in &boundary.permits {
            permit.store(false, Ordering::Release);
        }
    }

    fn record_edit(&mut self) {
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.control_sync.clear();
    }

    fn move_row(&mut self, from: usize, to: usize) {
        if from < to && to < self.row_values.len() {
            self.row_values[from..=to].rotate_left(1);
        } else if to < from && from < self.row_values.len() {
            self.row_values[to..=from].rotate_right(1);
        }
        self.control_sync.clear();
    }

    fn projection_current(&self, request: &MouseCurrentDpiRequested) -> bool {
        self.owns_completion(request)
            && request.connection_key == self.connection_key
            && request.edit_revision == self.edit_revision
    }

    fn accept_report(
        &mut self,
        request: &MouseCurrentDpiRequested,
        report: &DpiReport,
    ) -> Option<[RowUpdate; 5]> {
        let updates = self.observed.accept(request.selection.as_ref(), report);
        let errors = [report.write_errors.stages(), report.write_errors.active()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("; ");
        self.read_error = (!errors.is_empty()).then_some(errors);
        self.observe_rows(request, updates)
    }

    fn observe_rows(
        &mut self,
        request: &MouseCurrentDpiRequested,
        updates: [RowUpdate; 5],
    ) -> Option<[RowUpdate; 5]> {
        if !self.projection_current(request) {
            return None;
        }
        for (value, update) in self.row_values.iter_mut().zip(updates) {
            match update {
                RowUpdate::Unchanged => {}
                RowUpdate::Unobserved => *value = None,
                RowUpdate::Observed(observed) => *value = Some(observed),
            }
        }
        Some(updates)
    }

    fn owns_completion(&self, request: &MouseCurrentDpiRequested) -> bool {
        self.pending.as_ref().is_some_and(|pending| {
            pending.sequence == request.sequence
                && pending.scope == request.scope
                && pending.connection_key == request.connection_key
                && Arc::ptr_eq(&pending.valid, &request.valid)
        })
    }

    fn release(&mut self, request: &MouseCurrentDpiRequested) -> bool {
        if !self.owns_completion(request) {
            return false;
        }
        if let Some(pending) = self.pending.take() {
            pending.cancel();
        }
        true
    }

    fn request_refresh(&mut self) {
        self.refresh_requested = true;
    }

    fn install_connection(
        &mut self,
        key: Option<String>,
        scope: Option<MousePollingScope>,
    ) -> bool {
        if self.connection_key == key && self.connection_scope == scope {
            return false;
        }
        self.cancel();
        self.connection_key = key;
        self.connection_scope = scope;
        true
    }

    fn initialize_connection(
        &mut self,
        input: ProfileInitialization<'_>,
    ) -> Option<MouseCurrentDpiRequested> {
        let changed = self.install_connection(input.connection_key, input.scope);
        self.macro_recording = input.macro_recording;
        if input.macro_recording {
            if let Some(pending) = &self.pending {
                pending.cancel();
            }
            if let Some(queued) = &mut self.queued {
                queued.change = None;
            }
            if let Some(initialization) = self.initialization_queued.take() {
                initialization.cancel();
            }
            self.initialization_key = None;
            if changed && self.connection_key.is_some() {
                self.request_refresh();
            }
            return None;
        }
        self.connection_key.as_ref()?;
        if let (Some(scope), Some(stages), false) =
            (input.scope, input.stages, input.profile_guid.is_empty())
        {
            return self.initialize_profile(scope, stages, input.profile_guid, input.selection);
        }
        if changed {
            self.request_refresh();
        }
        None
    }

    /// Install the connection first, then retain the source initializer and its
    /// following GET behind any predecessor's cancellation/cleanup barrier.
    fn initialize_profile(
        &mut self,
        scope: MousePollingScope,
        stages: DpiStagesDraft,
        profile_guid: &str,
        selection: Option<Selection>,
    ) -> Option<MouseCurrentDpiRequested> {
        let key = self.connection_key.clone()?;
        if profile_guid.is_empty() || self.initialization_key.as_ref() == Some(&key) {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let request = MouseCurrentDpiRequested {
            scope,
            sequence: self.sequence,
            profile: None,
            profile_save: Arc::new(AtomicBool::new(true)),
            change: Some(DpiChange {
                stages: Some(stages.into()),
                active: None,
                profile_guid: Some(profile_guid.into()),
                initialize: true,
            }),
            read: false,
            cleanup: false,
            drain_on_exit: false,
            restoration_ticket: self.temporary_restoration.ticket(),
            connection_key: Some(key.clone()),
            edit_revision: self.edit_revision,
            selection,
            valid: Arc::new(AtomicBool::new(true)),
        };
        let ready = self.enqueue_initialization(request);
        self.initialization_key = Some(key);
        self.request_refresh();
        ready
    }

    fn enqueue_initialization(
        &mut self,
        request: MouseCurrentDpiRequested,
    ) -> Option<MouseCurrentDpiRequested> {
        if self.pending.is_some() {
            if let Some(previous) = self.initialization_queued.replace(request) {
                self.finish_initialization(&previous, false);
                previous.cancel();
            }
            None
        } else {
            self.enqueue(request)
        }
    }

    fn request_identity(&mut self, scope: MousePollingScope) -> Option<MouseCurrentDpiRequested> {
        if self
            .pending
            .iter()
            .chain(self.initialization_queued.iter())
            .any(|request| {
                request.is_valid()
                    && request.scope == scope
                    && request.connection_key == self.connection_key
                    && request
                        .change
                        .as_ref()
                        .is_some_and(DpiChange::is_identity_read)
            })
        {
            return None;
        }
        self.local_profile_known = false;
        self.local_profile_error = None;
        self.sequence = self.sequence.wrapping_add(1);
        self.enqueue_initialization(MouseCurrentDpiRequested {
            scope,
            sequence: self.sequence,
            profile: None,
            profile_save: Arc::new(AtomicBool::new(true)),
            change: Some(DpiChange {
                stages: None,
                active: None,
                profile_guid: None,
                initialize: true,
            }),
            read: true,
            cleanup: false,
            drain_on_exit: false,
            restoration_ticket: self.temporary_restoration.ticket(),
            connection_key: self.connection_key.clone(),
            edit_revision: self.edit_revision,
            selection: None,
            valid: Arc::new(AtomicBool::new(true)),
        })
    }

    fn finish_initialization(&mut self, request: &MouseCurrentDpiRequested, succeeded: bool) {
        if !succeeded
            && request
                .change
                .as_ref()
                .is_some_and(|change| change.initialize && !change.is_identity_read())
            && self.connection_scope == Some(request.scope)
            && request.connection_key == self.initialization_key
        {
            self.initialization_key = None;
        }
    }

    fn take_queued(
        &mut self,
        scope: Option<MousePollingScope>,
    ) -> Option<MouseCurrentDpiRequested> {
        if self.pending.is_some() {
            return None;
        }
        if let Some(initialization) = self.initialization_queued.take() {
            if initialization.is_valid()
                && scope == Some(initialization.scope)
                && initialization.connection_key == self.connection_key
            {
                self.pending = Some(initialization.clone());
                return Some(initialization);
            }
            initialization.cancel();
            self.finish_initialization(&initialization, false);
        }
        if let Some(next) = self.queued.take() {
            if next.is_valid()
                && scope == Some(next.scope)
                && next.connection_key == self.connection_key
            {
                self.pending = Some(next.clone());
                return Some(next);
            }
            next.cancel();
        }
        if let Some(exit) = self.exit_request.take() {
            if exit.is_valid()
                && scope == Some(exit.scope)
                && exit.connection_key == self.connection_key
            {
                self.pending = Some(exit.clone());
                return Some(exit);
            }
            exit.cancel();
        }
        None
    }

    fn prepare_exit(
        &mut self,
        scope: Option<MousePollingScope>,
        restore: Option<DpiStagesDraft>,
        selection: Option<Selection>,
    ) -> Result<Option<MouseCurrentDpiRequested>, String> {
        let key = self.connection_key.clone();
        let connection_scope = self.connection_scope;
        let previous = self
            .queued
            .iter()
            .chain(self.pending.iter())
            .find(|request| {
                request.is_valid() && Some(request.scope) == scope && request.connection_key == key
            });
        let profile = previous.and_then(|request| request.profile().cloned());
        let profile_save = previous
            .map(|request| request.profile_save.clone())
            .unwrap_or_else(|| Arc::new(AtomicBool::new(true)));
        // Invalidation requests cancellation but the old pending request remains
        // the serialization barrier until its worker has released the transport.
        self.invalidate(true);
        self.connection_key = key;
        self.connection_scope = connection_scope;
        let current_scope = scope.filter(|scope| Some(*scope) == connection_scope);
        let Some(scope) = current_scope.filter(|_| self.connection_key.is_some()) else {
            return if restore.is_some() {
                Err("DPI clutch restoration has no current connection identity".into())
            } else {
                Ok(None)
            };
        };
        if profile.is_none() && restore.is_none() {
            return Ok(None);
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.exit_request = Some(MouseCurrentDpiRequested {
            scope,
            sequence: self.sequence,
            profile,
            profile_save,
            cleanup: restore.is_some(),
            drain_on_exit: false,
            restoration_ticket: self.temporary_restoration.ticket(),
            change: restore.map(|stages| DpiChange {
                stages: Some(stages.into()),
                active: None,
                profile_guid: None,
                initialize: false,
            }),
            read: false,
            connection_key: self.connection_key.clone(),
            edit_revision: self.edit_revision,
            selection,
            valid: Arc::new(AtomicBool::new(true)),
        });
        Ok(self.take_queued(Some(scope)))
    }

    fn take_ready_refresh(&mut self) -> bool {
        if self.pending.is_some() || !self.refresh_requested {
            return false;
        }
        self.refresh_requested = false;
        self.read_error = None;
        true
    }

    pub(super) fn cancel(&mut self) {
        self.invalidate(false);
    }

    fn invalidate(&mut self, retain_accepted: bool) {
        if let Some(pending) = &self.pending {
            if !retain_accepted || !pending.drain_on_exit {
                pending.cancel();
            }
        }
        if let Some(queued) = self.queued.take() {
            if retain_accepted && queued.drain_on_exit {
                self.queued = Some(queued);
            } else {
                queued.cancel();
            }
        }
        if let Some(initialization) = self.initialization_queued.take() {
            initialization.cancel();
        }
        if let Some(exit) = self.exit_request.take() {
            exit.cancel();
        }
        self.observed = Observation::default();
        self.row_values = [None; 5];
        self.control_sync.clear();
        self.connection_key = None;
        self.connection_scope = None;
        self.initialization_key = None;
        self.refresh_requested = false;
        self.read_error = None;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl EventEmitter<MouseCurrentDpiRequested> for MouseProductWorkspace {}

impl MouseProductWorkspace {
    pub fn current_dpi_profile_save_boundary(
        &self,
        scope: MousePollingScope,
        cx: &App,
    ) -> Option<MouseCurrentDpiProfileSaveBoundary> {
        (self.mouse_polling_scope(cx) == Some(scope))
            .then(|| self.current_dpi.profile_save_boundary(scope))
            .flatten()
    }

    pub fn current_dpi_matcher_stages_saved(
        &mut self,
        boundary: &MouseCurrentDpiProfileSaveBoundary,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_polling_scope(cx) == Some(boundary.scope) {
            self.current_dpi.matcher_stages_saved(boundary);
        }
    }

    /// Matcher uses the same device-task owner as ordinary DPI edits. Its
    /// separate source task maker has already saved the profile before admission.
    pub(super) fn enqueue_matcher_stages(
        &mut self,
        stages: razer_device::mouse_current_dpi::SourceStages,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let scope = self
            .mouse_polling_scope(cx)
            .context("Matcher DPI has no current page scope")?;
        ensure!(
            self.current_dpi.connection_key.is_some(),
            "Matcher DPI has no current connection"
        );
        self.current_dpi.record_edit();
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        let selection = Selection::capture_source(self.spec.product_id, &stages);
        let request = MouseCurrentDpiRequested {
            scope,
            sequence: self.current_dpi.sequence,
            profile: None,
            profile_save: Arc::new(AtomicBool::new(true)),
            change: (!self.current_dpi.macro_recording).then_some(DpiChange {
                stages: Some(stages),
                active: None,
                profile_guid: matches!(self.spec.product_id, 182 | 190)
                    .then(|| self.draft["guid"].as_str().unwrap_or("").to_owned()),
                initialize: false,
            }),
            read: false,
            cleanup: false,
            drain_on_exit: true,
            restoration_ticket: self.current_dpi.temporary_restoration.ticket(),
            connection_key: self.current_dpi.connection_key.clone(),
            edit_revision: self.current_dpi.edit_revision,
            selection,
            valid: Arc::new(AtomicBool::new(true)),
        };
        self.enqueue_current_dpi(request, cx);
        Ok(())
    }
    pub fn current_dpi_profile_read_matches(
        &self,
        request: &MouseCurrentDpiRequested,
        cx: &App,
    ) -> bool {
        self.current_dpi_request_matches(request, cx)
            && self.current_dpi.projection_current(request)
    }
    pub fn restore_current_dpi_local_profile(
        &mut self,
        result: Result<Option<DpiStagesDraft>, String>,
        cx: &mut Context<Self>,
    ) {
        if !razer_storage::host_local_profile::supported(self.spec.product_id) {
            return;
        }
        self.current_dpi.local_profile_known = false;
        self.current_dpi.local_profile_error = None;
        match result {
            Ok(Some(stages)) => {
                self.draft["dpiStages"] = razer_storage::host_local_profile::stage_value(&stages);
                self.current_dpi.local_profile_known = true;
            }
            Ok(None) => {}
            Err(error) => self.current_dpi.local_profile_error = Some(error),
        }
        cx.notify();
    }
    pub(super) fn current_dpi_profile_known(&self) -> bool {
        !razer_storage::host_local_profile::supported(self.spec.product_id)
            || self.current_dpi.local_profile_known
    }
    pub fn observe_current_dpi_macro_recording(&mut self, recording: bool) {
        if self.spec.product_id != 182 {
            return;
        }
        if recording && !self.current_dpi.macro_recording {
            if let Some(request) = &self.current_dpi.pending {
                request.cancel();
            }
            if let Some(queued) = &mut self.current_dpi.queued {
                queued.change = None;
            }
            if let Some(initialization) = self.current_dpi.initialization_queued.take() {
                initialization.cancel();
            }
            self.current_dpi.initialization_key = None;
        }
        self.current_dpi.macro_recording = recording;
    }
    /// Called after the source profile/initialization owners have selected the
    /// current physical route and profile. A busy worker retains the refresh.
    pub fn observe_current_dpi_connection(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        let scope = self.mouse_polling_scope(cx);
        if key.is_none() {
            self.current_dpi.local_profile_known = false;
            self.current_dpi.local_profile_error = None;
        }
        if !self.current_dpi.install_connection(key, scope) {
            return;
        }
        if self.current_dpi.connection_key.is_some() {
            self.request_current_dpi_read(cx);
        }
        cx.notify();
    }

    pub fn initialize_mouse182_profile(
        &mut self,
        key: Option<String>,
        profile_guid: &str,
        macro_recording: bool,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 182 {
            return;
        }
        let initialization = ProfileInitialization {
            connection_key: key.as_ref().map(|key| format!("{key}:{profile_guid}")),
            scope: self.mouse_polling_scope(cx),
            profile_guid,
            stages: self.current_dpi_draft(),
            selection: self.current_dpi_selection(),
            macro_recording: macro_recording || self.current_dpi.macro_recording,
        };
        let request = self.current_dpi.initialize_connection(initialization);
        self.initialize_current_mouse_polling(key.as_deref(), profile_guid, cx);
        if let Some(request) = request {
            cx.emit(request);
        }
        self.start_current_dpi_refresh(cx);
        cx.notify();
    }
    pub fn apply_current_dpi_profile(&mut self, cx: &mut Context<Self>) {
        self.submit_current_dpi(Edit::Profile, cx);
    }

    /// Current158 mapping callbacks distinguish the profile stage task from
    /// temporary clutch DPI. Clutch never becomes a saved profile value.
    pub(super) fn submit_receiver_callback_dpi(
        &mut self,
        temporary: Option<DpiPair>,
        persist: bool,
        transmit: bool,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 158 {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let stages = self.current_dpi_draft();
        if (temporary.is_none() || persist) && stages.is_none() {
            return;
        }
        if persist {
            self.current_dpi.record_edit();
        }
        if temporary.is_some() && transmit && !self.current_dpi.macro_recording {
            self.current_dpi.temporary_restoration.require();
        }
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        let request = MouseCurrentDpiRequested {
            scope,
            sequence: self.current_dpi.sequence,
            profile: persist.then(|| self.draft.clone()),
            profile_save: Arc::new(AtomicBool::new(true)),
            change: (transmit && !self.current_dpi.macro_recording)
                .then(|| receiver_callback_change(temporary, stages))
                .flatten(),
            read: false,
            cleanup: false,
            drain_on_exit: false,
            restoration_ticket: self.current_dpi.temporary_restoration.ticket(),
            connection_key: self.current_dpi.connection_key.clone(),
            edit_revision: self.current_dpi.edit_revision,
            selection: if temporary.is_some() {
                None
            } else {
                self.current_dpi_selection()
            },
            valid: Arc::new(AtomicBool::new(true)),
        };
        self.enqueue_current_dpi(request, cx);
    }

    pub(super) fn receiver_dpi_callbacks_blocked(&self) -> bool {
        self.current_dpi.macro_recording
    }
    fn current_dpi_draft(&self) -> Option<DpiStagesDraft> {
        if !self.current_dpi_profile_known() {
            return None;
        }
        let stages = self
            .draft
            .pointer(self.spec.stages_path())?
            .as_array()?
            .iter()
            .map(|stage| {
                Some(DpiStage {
                    x: u16::try_from(stage[self.spec.dpi_axis(0)].as_u64()?).ok()?,
                    y: u16::try_from(stage[self.spec.dpi_axis(1)].as_u64()?).ok()?,
                    visible: stage[self.spec.visible_key()].as_bool()?,
                    independent: stage[self.spec.independent_key()].as_bool()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(DpiStagesDraft {
            enabled: self
                .draft
                .pointer(self.spec.stage_enable_path())?
                .as_bool()?,
            active_stage: usize::try_from(self.draft.pointer(self.spec.active_path())?.as_u64()?)
                .ok()?,
            stages,
        })
    }

    fn current_dpi_selection(&self) -> Option<Selection> {
        Selection::capture(self.spec.product_id, &self.current_dpi_draft()?)
    }

    pub(super) fn submit_current_dpi_field(&mut self, path: &str, cx: &mut Context<Self>) {
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none()
            || !(path.starts_with(self.spec.stages_path())
                || path == self.spec.active_path()
                || path == self.spec.stage_enable_path())
            || path.ends_with(self.spec.independent_key())
        {
            return;
        }
        let edit = if path == self.spec.active_path() {
            Edit::SelectStage
        } else {
            Edit::Stages
        };
        self.submit_current_dpi(edit, cx);
    }

    pub(super) fn submit_current_dpi(&mut self, edit: Edit, cx: &mut Context<Self>) {
        let Tasks { stages, active } = edit.tasks(self.spec.product_id);
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none() {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let Some(draft) = self.current_dpi_draft() else {
            return;
        };
        self.current_dpi.record_edit();
        let selected = draft
            .active_stage
            .checked_sub(1)
            .and_then(|index| draft.stages.get(index))
            .map(|row| DpiPair { x: row.x, y: row.y });
        if active {
            if let Some(selected) = selected {
                if self.draft.get("DPI").is_some() {
                    self.draft["DPI"] = json!({"X":selected.x,"Y":selected.y});
                } else {
                    self.draft["dpi"] = json!({"x":selected.x,"y":selected.y});
                }
            }
        }
        if self.draft.get("DPIStages").is_some() {
            self.draft["DPIStages"]["StageNum"] =
                json!(draft.stages.iter().filter(|stage| stage.visible).count());
        } else {
            self.draft["dpiStages"]["count"] =
                json!(draft.stages.iter().filter(|stage| stage.visible).count());
        }
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        let old = self
            .current_dpi
            .queued
            .as_ref()
            .and_then(|request| request.change.as_ref());
        let stage_change = stages || old.is_some_and(|change| change.stages.is_some());
        let active_change = active || old.is_some_and(|change| change.active.is_some());
        let change =
            (!self.current_dpi.macro_recording && (stage_change || active_change)).then(|| {
                DpiChange {
                    stages: stage_change.then(|| draft.into()),
                    active: active_change.then_some(selected).flatten(),
                    profile_guid: matches!(self.spec.product_id, 182 | 190).then(|| {
                        self.draft
                            .get("guid")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    }),
                    initialize: false,
                }
            });
        let request = MouseCurrentDpiRequested {
            scope,
            sequence: self.current_dpi.sequence,
            profile: Some(self.draft.clone()),
            profile_save: Arc::new(AtomicBool::new(true)),
            change,
            read: false,
            cleanup: false,
            drain_on_exit: false,
            restoration_ticket: self.current_dpi.temporary_restoration.ticket(),
            connection_key: self.current_dpi.connection_key.clone(),
            edit_revision: self.current_dpi.edit_revision,
            selection: self.current_dpi_selection(),
            valid: Arc::new(AtomicBool::new(true)),
        };
        self.enqueue_current_dpi(request, cx);
    }

    pub fn request_current_dpi_read(&mut self, cx: &mut Context<Self>) {
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none() {
            return;
        }
        self.current_dpi.request_refresh();
        self.start_current_dpi_refresh(cx);
    }

    fn start_current_dpi_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if !self.current_dpi.take_ready_refresh() {
            return;
        }
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        self.enqueue_current_dpi(
            MouseCurrentDpiRequested {
                scope,
                sequence: self.current_dpi.sequence,
                profile: None,
                profile_save: Arc::new(AtomicBool::new(true)),
                change: None,
                read: true,
                cleanup: false,
                drain_on_exit: false,
                restoration_ticket: self.current_dpi.temporary_restoration.ticket(),
                connection_key: self.current_dpi.connection_key.clone(),
                edit_revision: self.current_dpi.edit_revision,
                selection: self.current_dpi_selection(),
                valid: Arc::new(AtomicBool::new(true)),
            },
            cx,
        );
    }

    /// Source90012.gn obtains the protocol serial before77846 reads the
    /// serial-indexed profile cache. This request contains no setting or
    /// profile, so the transport must perform only the identity getter.
    pub fn request_current_dpi_identity(&mut self, cx: &mut Context<Self>) {
        if !razer_storage::host_local_profile::supported(self.spec.product_id) {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if let Some(request) = self.current_dpi.request_identity(scope) {
            cx.emit(request);
        }
    }

    fn enqueue_current_dpi(&mut self, request: MouseCurrentDpiRequested, cx: &mut Context<Self>) {
        if let Some(request) = self.current_dpi.enqueue(request) {
            cx.emit(request);
        }
    }

    pub fn current_dpi_request_matches(
        &self,
        request: &MouseCurrentDpiRequested,
        cx: &App,
    ) -> bool {
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && self.current_dpi.connection_key == request.connection_key
            && self.current_dpi.owns_completion(request)
    }

    pub fn finish_current_dpi(
        &mut self,
        request: &MouseCurrentDpiRequested,
        observed: Option<DpiReport>,
        cx: &mut Context<Self>,
    ) {
        self.accept_current_dpi_result(request, Ok(observed), cx);
    }

    fn accept_current_dpi_result(
        &mut self,
        request: &MouseCurrentDpiRequested,
        result: Result<Option<DpiReport>, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.current_dpi.owns_completion(request) {
            return;
        }
        let current = self.current_dpi_request_matches(request, cx);
        let temporary_clutch = self.spec.product_id == 158
            && request.profile.is_none()
            && request
                .change
                .as_ref()
                .is_some_and(|change| change.active.is_some());
        let initialized = current
            && result
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .is_some_and(|report| report.verified);
        self.current_dpi.finish_initialization(request, initialized);
        self.current_dpi
            .confirm_temporary_restore(request, initialized);
        if request.cleanup
            && initialized
            && request.restoration_ticket == self.current_dpi.temporary_restoration.ticket()
        {
            self.receiver_mapping.finish_dpi_restore();
        }
        if current {
            match result {
                Ok(Some(report)) => {
                    let mut projection = request.clone();
                    if !temporary_clutch
                        && (projection.selection.is_none()
                            || projection.is_read()
                                && razer_storage::host_local_profile::supported(
                                    self.spec.product_id,
                                ))
                        && self.current_dpi.projection_current(request)
                    {
                        projection.selection = self.current_dpi_selection();
                    }
                    if let Some(values) = self.current_dpi.accept_report(&projection, &report) {
                        self.apply_current_dpi_observation(values, cx);
                    }
                }
                Err(error) => {
                    self.current_dpi.read_error = Some(error);
                    self.current_dpi.observed = Observation::default();
                    if !temporary_clutch
                        && let Some(updates) = self
                            .current_dpi
                            .observe_rows(request, [RowUpdate::Unobserved; 5])
                    {
                        self.apply_current_dpi_observation(updates, cx);
                    }
                }
                Ok(None) => {}
            }
        }
        self.current_dpi.release(request);
        let scope = self.mouse_polling_scope(cx);
        if let Some(next) = self.current_dpi.take_queued(scope) {
            cx.emit(next);
        }
        self.start_current_dpi_refresh(cx);
        cx.notify();
    }

    pub fn finish_current_dpi_result(
        &mut self,
        request: &MouseCurrentDpiRequested,
        result: Result<DpiReport, String>,
        cx: &mut Context<Self>,
    ) {
        self.accept_current_dpi_result(request, result.map(Some), cx);
    }

    fn current_dpi_row_has_preview(&self, index: usize, cx: &App) -> bool {
        if self.dpi_dragged_row.is_some() {
            return true;
        }
        for axis in 0..2 {
            let path = format!(
                "{}/{index}/{}",
                self.spec.stages_path(),
                self.spec.dpi_axis(axis)
            );
            if self
                .dpi_numbers
                .get(&path)
                .is_some_and(|state| state.registered || state.typed)
            {
                return true;
            }
            if self.current_dpi.control_sync.contains_key(&path) {
                continue;
            }
            if self
                .sliders
                .get(&path)
                .is_some_and(|slider| slider.read(cx).value().start() != self.number(&path))
                || self.inputs.get(&path).is_some_and(|input| {
                    input.read(cx).value().to_string() != (self.number(&path) as i64).to_string()
                })
            {
                return true;
            }
        }
        self.dpi_single_sliders.get(&index).is_some_and(|slider| {
            let path = format!(
                "{}/{index}/{}",
                self.spec.stages_path(),
                self.spec.dpi_axis(0)
            );
            !self.current_dpi.control_sync.contains_key(&path)
                && slider.read(cx).value().start() != self.number(&path)
        })
    }

    fn apply_current_dpi_observation(&mut self, values: [RowUpdate; 5], cx: &App) {
        for (index, update) in values.into_iter().enumerate() {
            let value = match update {
                RowUpdate::Unchanged => continue,
                RowUpdate::Unobserved => {
                    for axis in 0..2 {
                        self.current_dpi.control_sync.remove(&format!(
                            "{}/{index}/{}",
                            self.spec.stages_path(),
                            self.spec.dpi_axis(axis)
                        ));
                    }
                    continue;
                }
                RowUpdate::Observed(value) => value,
            };
            if self.current_dpi_row_has_preview(index, cx) {
                continue;
            }
            for (axis, value) in [value.x, value.y].into_iter().enumerate() {
                let path = format!(
                    "{}/{index}/{}",
                    self.spec.stages_path(),
                    self.spec.dpi_axis(axis)
                );
                if self.draft.pointer(&path).is_some() {
                    set_pointer(&mut self.draft, &path, json!(value));
                    self.current_dpi.control_sync.insert(path, value);
                }
            }
        }
    }

    /// Domain acceptance and worker settlement do not require a live window.
    /// Only the retained input/slider synchronization uses this optional step.
    pub fn sync_current_dpi_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let updates = std::mem::take(&mut self.current_dpi.control_sync);
        self.syncing = true;
        for (path, value) in updates {
            if self
                .dpi_numbers
                .get(&path)
                .is_some_and(|state| state.registered || state.typed)
            {
                continue;
            }
            if let Some(input) = self.inputs.get(&path) {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx)
                });
            }
            if let Some(slider) = self.sliders.get(&path) {
                slider.update(cx, |slider, cx| {
                    slider.set_value(f32::from(value), window, cx)
                });
            }
            if let Some(grid) = self.dpi_grids.get(&path) {
                grid.update(cx, |grid, cx| grid.reset(f32::from(value), window, cx));
            }
            if let Some((base, axis)) = path.rsplit_once('/') {
                if axis == self.spec.dpi_axis(0) {
                    if let Some(single) = base
                        .rsplit('/')
                        .next()
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| self.dpi_single_sliders.get(&index))
                    {
                        single.update(cx, |single, cx| {
                            single.set_value(f32::from(value), window, cx)
                        });
                    }
                }
            }
        }
        self.syncing = false;
        cx.notify();
    }

    pub(super) fn current_dpi_row_available(&self, index: usize) -> bool {
        razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none()
            || self
                .current_dpi
                .row_values
                .get(index)
                .is_some_and(Option::is_some)
    }

    pub(super) fn move_current_dpi_row(&mut self, from: usize, to: usize) {
        self.current_dpi.move_row(from, to);
    }

    pub(super) fn current_dpi_unknown_number(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .w(surface::css(62.))
            .h(surface::css(26.))
            .ml(surface::css(10.))
            .pl(surface::css(5.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .text_color(cx.theme().muted_foreground)
            .child("—")
            .into_any_element()
    }

    pub(super) fn current_dpi_unknown_track(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .w(surface::css(250.))
            .h(surface::css(20.))
            .ml(surface::css(10.))
            .flex()
            .items_center()
            .child(div().w_full().h_px().bg(cx.theme().border))
            .into_any_element()
    }

    pub(super) fn current_dpi_status(&self, cx: &Context<Self>) -> Option<AnyElement> {
        razer_device::mouse_current_dpi::capability(self.spec.product_id)?;
        let error = self
            .current_dpi
            .read_error
            .iter()
            .chain(self.current_dpi.local_profile_error.iter())
            .chain(self.receiver_mapping.error.iter())
            .map(String::as_str)
            .chain(self.current_dpi.observed.errors())
            .collect::<Vec<_>>()
            .join("; ");
        let loading = self.current_dpi.refresh_requested
            || self
                .current_dpi
                .pending
                .as_ref()
                .is_some_and(|request| request.read && request.change.is_none());
        let connected = self.current_dpi.connection_key.is_some();
        if error.is_empty()
            && !loading
            && connected
            && self.current_dpi.row_values.iter().any(Option::is_some)
        {
            return None;
        }
        let label = if !error.is_empty() {
            format!("DPI: {error}")
        } else if loading {
            t("LOADING")
        } else if !connected {
            t("DISCONNECTED")
        } else {
            "DPI: —".into()
        };
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .mb_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label)
                .children((connected && !loading).then(|| {
                    Button::new("mouse-current-dpi-retry")
                        .ghost()
                        .small()
                        .label(t("RETRY"))
                        .on_click(cx.listener(|this, _, _, cx| this.request_current_dpi_read(cx)))
                }))
                .into_any_element(),
        )
    }

    /// Closing drains the newest requested profile and the current connection's
    /// held clutch restoration through their existing serialized owners.
    pub fn prepare_current_dpi_exit(
        &mut self,
        cx: &App,
    ) -> Result<Option<MouseCurrentDpiRequested>, String> {
        if self.spec.product_id == 158 {
            self.receiver_mapping.close_callback_admission();
        }
        let restore = if self.spec.product_id == 158
            && (self.receiver_mapping.needs_dpi_restore()
                || self.current_dpi.temporary_restoration.required())
        {
            Some(self.current_dpi_draft().ok_or_else(|| {
                "DPI clutch restoration has no valid current profile stages".to_owned()
            })?)
        } else {
            None
        };
        let scope = self.mouse_polling_scope(cx);
        let selection = self.current_dpi_selection();
        self.current_dpi.prepare_exit(scope, restore, selection)
    }

    pub fn current_dpi_exit_pending(&self) -> bool {
        self.current_dpi.pending.is_some()
            || self.current_dpi.queued.is_some()
            || self.current_dpi.initialization_queued.is_some()
            || self.current_dpi.exit_request.is_some()
    }
}

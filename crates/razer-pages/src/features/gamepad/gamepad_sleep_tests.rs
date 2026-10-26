use super::{
    ControllerSleepObservation, ControllerSleepRequested, PowerSaving, SleepObservation,
    SleepOperation, SleepResult, SleepRuntime,
};
use razer_device::controller_sleep::{SleepCompletion, SleepReadStatus, SleepWriteStatus};
use std::prelude::v1::test;

fn setting(minutes: u16) -> PowerSaving {
    PowerSaving::new(minutes, true).unwrap()
}
fn observed(seconds: u16) -> SleepResult {
    SleepResult::new(
        &SleepOperation::Read,
        SleepWriteStatus::NotRequested,
        SleepReadStatus::Observed {
            observation: SleepObservation::from_seconds(seconds),
        },
        SleepCompletion::Observed,
    )
    .unwrap()
}
fn save(runtime: &mut SleepRuntime, minutes: u16) -> ControllerSleepRequested {
    let request = runtime.prepare_storage(setting(minutes));
    assert!(request.requires_persistence());
    assert!(runtime.matches(&request));
    request
}

#[test]
fn controller_initialization_restoration_uses_current_raw_read_and_source_minute_projection() {
    let mut runtime = SleepRuntime::default();
    runtime.persisted = Some(setting(60));
    let read = razer_device::controller_initialization::Read::Observed {
        value: SleepObservation::from_seconds(301),
    };
    assert!(runtime.restore_import(&read, false));
    assert_eq!(
        runtime.observation,
        ControllerSleepObservation::Observed(SleepObservation::from_seconds(301))
    );
    let request = runtime.next().unwrap();
    assert!(request.initialization_restore);
    assert!(!request.requires_persistence());
    assert_eq!(request.setting(), Some(setting(5)));
    let confirmed = SleepResult::new(
        request.operation(),
        SleepWriteStatus::Acknowledged,
        SleepReadStatus::Observed {
            observation: SleepObservation::from_seconds(300),
        },
        SleepCompletion::Confirmed,
    )
    .unwrap();
    request.set_cleanup_pending(true);
    assert!(runtime.finish(&request, Some(confirmed.clone()), None));
    assert_eq!(
        runtime.observation,
        ControllerSleepObservation::Observed(SleepObservation::from_seconds(300))
    );
    assert!(
        runtime.active.is_some(),
        "A real result does not release an unconfirmed cleanup owner"
    );
    request.set_cleanup_pending(false);
    assert!(runtime.finish(&request, Some(confirmed), None));
    assert!(runtime.active.is_none());
}

#[test]
fn controller_initialization_failed_missing_and_macro_held_roots_do_not_restore_cached_defaults() {
    for read in [
        razer_device::controller_initialization::Read::Unobserved,
        razer_device::controller_initialization::Read::Failed {
            error: "real root getter failed".into(),
        },
    ] {
        let mut runtime = SleepRuntime::default();
        runtime.persisted = Some(setting(60));
        assert!(!runtime.restore_import(&read, false));
        assert!(runtime.next().is_none());
        assert_eq!(runtime.persisted, Some(setting(60)));
        assert!(runtime.presentation().is_none());
    }
    let mut runtime = SleepRuntime::default();
    assert!(!runtime.restore_import(
        &razer_device::controller_initialization::Read::Observed {
            value: SleepObservation::from_seconds(301)
        },
        true
    ));
    assert!(runtime.next().is_none());
    assert_eq!(runtime.presentation(), Some(setting(5)));
    assert_eq!(
        runtime.observation,
        ControllerSleepObservation::Observed(SleepObservation::from_seconds(301))
    );
}

#[test]
fn storage_is_independent_of_running_and_skipped_device_tasks() {
    let mut runtime = SleepRuntime::default();
    runtime.enqueue(SleepOperation::Read);
    let read = runtime.next().unwrap();
    let first = save(&mut runtime, 5);
    let second = save(&mut runtime, 15);
    let newest = save(&mut runtime, 60);
    assert!(runtime.matches(&first) && runtime.matches(&second) && runtime.matches(&newest));
    // Every storage continuation is accepted before the slow active GET ends.
    assert!(runtime.finish(&first, None, None));
    assert!(runtime.finish(&second, None, None));
    assert!(runtime.finish(&newest, None, None));
    assert!(runtime.storage_requests.is_empty());
    assert!(
        runtime.matches(&read),
        "Active source task is not superseded"
    );
    assert!(runtime.next().is_none());
    assert!(runtime.finish(&read, Some(observed(300)), None));
    let next = runtime.next().unwrap();
    assert_eq!(next.setting(), Some(setting(60)));
    assert!(
        !next.requires_persistence(),
        "Device dispatch must not save again"
    );
    assert!(
        runtime.pending.is_none(),
        "5 and 15 minute device tasks were skipped"
    );
}

#[test]
fn initial_read_failure_preserves_unobserved_state_without_default_write() {
    let mut runtime = SleepRuntime::default();
    runtime.persisted = Some(setting(5));
    assert!(
        runtime.presentation().is_none(),
        "A file is not a device observation"
    );
    runtime.enqueue(SleepOperation::Read);
    let read = runtime.next().unwrap();
    assert!(!runtime.enabled());
    let failure = SleepResult::new(
        &SleepOperation::Read,
        SleepWriteStatus::NotRequested,
        SleepReadStatus::Failed {
            error: "read failed".into(),
        },
        SleepCompletion::Failed,
    )
    .unwrap();
    runtime.finish(&read, Some(failure), None);
    assert!(matches!(
        runtime.observation,
        ControllerSleepObservation::Failed(_)
    ));
    assert!(runtime.presentation().is_none());
    assert!(!runtime.enabled());
    assert!(
        runtime.next().is_none(),
        "GET failure never creates a setter"
    );
    assert_eq!(runtime.persisted, Some(setting(5)));
}

#[test]
fn macro_recording_saves_each_root_setting_without_accessing_the_device() {
    let mut runtime = SleepRuntime::default();
    runtime.recording = true;
    let first = save(&mut runtime, 15);
    let next = save(&mut runtime, 30);
    assert!(!first.device_access() && !next.device_access());
    runtime.finish(&first, None, None);
    runtime.finish(&next, None, None);
    assert!(runtime.next().is_none());
    assert_eq!(runtime.observation, ControllerSleepObservation::Unobserved);
    // Recording can begin while the file writer is working.
    runtime.recording = false;
    let active_save = save(&mut runtime, 45);
    runtime.recording = true;
    runtime.finish(&active_save, None, None);
    assert!(runtime.next().is_none());
}

#[test]
fn reconnection_rejects_old_device_results_but_keeps_owned_storage_continuations() {
    let mut runtime = SleepRuntime::default();
    runtime.enqueue(SleepOperation::Read);
    let read = runtime.next().unwrap();
    let stored = save(&mut runtime, 15);
    runtime.invalidate(false);
    assert!(!read.is_valid());
    assert!(!runtime.finish(&read, Some(observed(900)), None));
    assert_eq!(
        runtime.observation,
        ControllerSleepObservation::Disconnected
    );
    assert!(
        runtime.matches(&stored),
        "File write belongs to the workspace lifetime"
    );
    assert!(runtime.finish(&stored, None, None));
    assert!(
        runtime.next().is_none(),
        "Saved old connection intent cannot write a new connection"
    );
    runtime.invalidate(true);
    runtime.enqueue(SleepOperation::Read);
    let current = runtime.next().unwrap();
    runtime.finish(&current, Some(observed(1800)), None);
    assert_eq!(runtime.presentation(), Some(setting(30)));
}

#[test]
fn mismatch_retains_real_readback_and_newer_optimistic_draft_separately() {
    let mut runtime = SleepRuntime::default();
    runtime.observation = ControllerSleepObservation::Observed(SleepObservation::from_seconds(300));
    let save = save(&mut runtime, 15);
    runtime.finish(&save, None, None);
    let write = runtime.next().unwrap();
    runtime.draft = Some(setting(30));
    let result = SleepResult::new(
        write.operation(),
        SleepWriteStatus::Acknowledged,
        SleepReadStatus::Observed {
            observation: SleepObservation::from_seconds(899),
        },
        SleepCompletion::Mismatch,
    )
    .unwrap();
    runtime.finish(&write, Some(result), None);
    assert_eq!(runtime.presentation(), Some(setting(30)));
    assert_eq!(
        runtime.observation,
        ControllerSleepObservation::Observed(SleepObservation::from_seconds(899))
    );
    assert_eq!(
        runtime.last_result.as_ref().unwrap().completion,
        SleepCompletion::Mismatch
    );
}

#[test]
fn storage_failure_does_not_enqueue_and_drop_cancels_both_owned_lifetimes() {
    let mut runtime = SleepRuntime::default();
    let request = save(&mut runtime, 15);
    runtime.finish(&request, None, Some("disk conflict".into()));
    assert!(runtime.next().is_none());
    assert_eq!(runtime.last_error.as_deref(), Some("disk conflict"));
    let request = save(&mut runtime, 30);
    let device_guard = request.guard();
    let storage_guard = request.storage_guard.clone();
    drop(runtime);
    assert!(!device_guard.load(std::sync::atomic::Ordering::Acquire));
    assert!(!storage_guard.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn current_controller_options_are_bare_minute_values() {
    for pid in [2636, 2676] {
        let spec = super::super::source_product(pid).unwrap();
        assert_eq!(spec.power_minutes, [5, 15, 30, 45, 60]);
        let labels = spec
            .power_minutes
            .iter()
            .map(|value| super::super::power_saving_label(*value))
            .collect::<Vec<_>>();
        assert_eq!(labels, ["5", "15", "30", "45", "60"]);
    }
}

#[test]
fn explicit_power_reentry_allows_one_read_after_failure_without_automatic_retry() {
    let mut runtime = SleepRuntime::default();
    runtime.read_requested = true;
    runtime.enqueue(SleepOperation::Read);
    let read = runtime.next().unwrap();
    runtime.enter_power();
    assert!(
        runtime.read_requested,
        "An in-flight GET is not duplicated on re-entry"
    );
    runtime.finish(&read, None, Some("actual GET failed".into()));
    assert!(
        runtime.read_requested,
        "Failure alone cannot start a retry loop"
    );
    assert!(runtime.next().is_none());
    runtime.enter_power();
    assert!(
        !runtime.read_requested,
        "Explicit re-entry permits the next single GET"
    );
    runtime.read_requested = true;
    runtime.enqueue(SleepOperation::Read);
    assert!(runtime.next().is_some());
    assert!(runtime.next().is_none());
}

#[test]
fn unknown_observed_seconds_loading_failure_and_disconnect_have_visible_status() {
    let mut runtime = SleepRuntime::default();
    let choices = [5, 15, 30, 45, 60];
    assert!(runtime.status(&choices).unwrap().contains('—'));
    runtime.observation = ControllerSleepObservation::Reading;
    assert_eq!(runtime.status(&choices).unwrap(), razer_i18n::t("LOADING"));
    runtime.observation = ControllerSleepObservation::Disconnected;
    assert_eq!(
        runtime.status(&choices).unwrap(),
        razer_i18n::t("DISCONNECTED")
    );
    runtime.observation = ControllerSleepObservation::Failed("transport failed".into());
    assert_eq!(runtime.status(&choices).unwrap(), razer_i18n::t("ERROR"));
    runtime.observation = ControllerSleepObservation::Observed(SleepObservation::from_seconds(301));
    let unknown = runtime.status(&choices).unwrap();
    assert!(
        unknown.contains(&razer_i18n::t_value("SEC", 301)),
        "Raw unknown seconds remain visible"
    );
    runtime.observation = ControllerSleepObservation::Observed(SleepObservation::from_seconds(420));
    assert!(
        runtime.status(&choices).is_some(),
        "An unlisted minute choice is not a default option"
    );
    runtime.observation = ControllerSleepObservation::Observed(SleepObservation::from_seconds(300));
    assert!(runtime.status(&choices).is_none());
}

#[test]
fn observed_result_does_not_release_active_queue_until_actual_cleanup_finishes() {
    let mut runtime = SleepRuntime::default();
    runtime.enqueue(SleepOperation::Read);
    let active = runtime.next().unwrap();
    let queued = save(&mut runtime, 15);
    runtime.finish(&queued, None, None);
    active.set_cleanup_pending(true);
    runtime.finish(
        &active,
        Some(observed(300)),
        Some("process still owned".into()),
    );
    assert!(runtime.matches(&active));
    assert!(runtime.next().is_none());
    assert_eq!(
        runtime.observation,
        ControllerSleepObservation::Observed(SleepObservation::from_seconds(300))
    );
    assert!(runtime.status(&[5, 15, 30, 45, 60]).is_some());
    active.set_cleanup_pending(false);
    runtime.finish(
        &active,
        Some(observed(300)),
        Some("process termination retry succeeded after failure".into()),
    );
    assert!(!runtime.matches(&active));
    assert_eq!(runtime.next().unwrap().setting(), Some(setting(15)));
}

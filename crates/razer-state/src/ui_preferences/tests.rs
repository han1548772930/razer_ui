use super::*;

fn observed(value: Option<bool>) -> Observation {
    value
        .map(Observation::current)
        .unwrap_or_else(Observation::missing)
}

fn finish_poll(core: &mut Core, work: &Work, result: Result<Option<&str>, &str>) {
    let Operation::PollNotifications { generation } = work.operation else {
        panic!("expected a mounted notification-key poll")
    };
    assert!(
        core.complete(Completed::PollNotifications {
            generation,
            result: result
                .map(|value| value.map(str::to_owned))
                .map_err(str::to_owned),
        })
        .is_none()
    );
}

fn mount_notifications(core: &mut Core, raw: &str, value: bool) {
    core.observe_notifications();
    let poll = core.queue.pop_front().unwrap();
    finish_poll(core, &poll, Ok(Some(raw)));
    let read = core.queue.pop_front().unwrap();
    finish_read(core, &read, Ok(observed(Some(value))));
    let write = core.queue.pop_front().unwrap();
    finish_write(core, &write, Ok(()));
}

#[test]
fn mounted_notification_listener_rereads_only_changed_key_and_runs_the_source_writer() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    mount_notifications(&mut core, "true", true);

    // Replacing the outer file, including another key's app metadata, does
    // not change this raw localStorage value or publish another writer.
    core.poll_notifications();
    core.poll_notifications();
    assert_eq!(
        core.queue.len(),
        1,
        "only one poll can be queued or executing"
    );
    let unchanged = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &unchanged, Ok(Some("true")));
    assert!(core.queue.is_empty());

    core.poll_notifications();
    let external = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &external, Ok(Some("false")));
    let read = core.queue.pop_front().unwrap();
    assert!(
        core.flags[&flag].value,
        "an event itself is not a completed read"
    );
    finish_read(&mut core, &read, Ok(observed(Some(false))));
    assert!(!core.flags[&flag].value);
    let write = core.queue.pop_front().unwrap();
    assert!(matches!(
        write.operation,
        Operation::Write { value: false, .. }
    ));
    finish_write(&mut core, &write, Ok(()));
    core.poll_notifications();
    let own_publication = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &own_publication, Ok(Some("false")));
    assert!(
        core.queue.is_empty(),
        "the observing document must not loop on its own write"
    );
}

#[test]
fn notification_listener_preserves_exact_strings_and_never_defaults_after_invalid_data() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    mount_notifications(&mut core, "true", true);
    for raw in [" true ", "invalid"] {
        core.poll_notifications();
        let poll = core.queue.pop_front().unwrap();
        finish_poll(&mut core, &poll, Ok(Some(raw)));
        let read = core.queue.pop_front().unwrap();
        if raw == "invalid" {
            finish_read(&mut core, &read, Err("invalid local boolean".into()));
            assert!(core.flags[&flag].value);
            assert!(core.queue.is_empty());
        } else {
            finish_read(&mut core, &read, Ok(observed(Some(true))));
            let write = core.queue.pop_front().unwrap();
            finish_write(&mut core, &write, Ok(()));
        }
    }
    core.poll_notifications();
    let still_invalid = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &still_invalid, Ok(Some("invalid")));
    assert!(
        core.queue.is_empty(),
        "unchanged invalid values are not a new storage event"
    );
}

#[test]
fn notification_file_read_failure_recovers_without_publishing_a_default() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    mount_notifications(&mut core, "false", false);
    core.poll_notifications();
    let failed = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &failed, Err("malformed outer file"));
    assert!(matches!(core.flags[&flag].load, LoadStatus::Failed(_)));
    assert_eq!(core.flags[&flag].persisted_value, Some(false));
    assert!(!core.flags[&flag].value);
    assert!(core.queue.is_empty());
    core.poll_notifications();
    let recovered = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &recovered, Ok(Some("false")));
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Ok(observed(Some(false))));
    let write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &write, Ok(()));
    assert_eq!(
        core.flags[&flag].load,
        LoadStatus::Observed(observed(Some(false)))
    );
}

#[test]
fn notification_unmount_and_remount_reject_an_old_poll_but_drain_accepted_writes() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    mount_notifications(&mut core, "true", true);
    core.poll_notifications();
    let old_poll = core.queue.pop_front().unwrap();
    core.set(flag, false);
    core.cancel_observation(flag);
    core.poll_notifications();
    assert_eq!(core.queue.len(), 1, "unmounted listeners admit no polls");
    core.observe_notifications();
    finish_poll(&mut core, &old_poll, Ok(Some("null")));
    assert!(
        core.notification_watch.pending,
        "old work cannot clear the successor's poll"
    );
    let accepted = core.queue.pop_front().unwrap();
    finish_write(&mut core, &accepted, Ok(()));
    let current_poll = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &current_poll, Ok(Some("false")));
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Ok(observed(Some(false))));
    assert!(!core.flags[&flag].value);
    let source_writer = core.queue.pop_front().unwrap();
    finish_write(&mut core, &source_writer, Ok(()));
    assert!(core.queue.is_empty());
}

#[test]
fn notification_exit_stops_polling_and_resume_uses_a_fresh_generation() {
    let mut core = Core::default();
    mount_notifications(&mut core, "true", true);
    core.poll_notifications();
    let old_poll = core.queue.pop_front().unwrap();
    core.begin_exit();
    core.poll_notifications();
    assert!(core.queue.is_empty());
    core.cancel_exit();
    finish_poll(&mut core, &old_poll, Ok(Some("false")));
    assert!(core.notification_watch.pending);
    let poll = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &poll, Ok(Some("false")));
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Ok(observed(Some(false))));
    assert!(!core.flags[&Flag::DisplayNotifications].value);
}

#[test]
fn external_notification_change_waits_for_accepted_click_and_invalidates_an_older_read() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    mount_notifications(&mut core, "true", true);
    core.flags.get_mut(&flag).unwrap().load = LoadStatus::NotRequested;
    core.load(flag);
    let older = core.queue.pop_front().unwrap();
    core.poll_notifications();
    let poll = core.queue.pop_front().unwrap();
    core.set(flag, false);
    finish_poll(&mut core, &poll, Ok(Some("false")));
    finish_read(&mut core, &older, Ok(observed(Some(true))));
    assert!(!core.flags[&flag].value);
    assert_eq!(core.flags[&flag].load, LoadStatus::Loading);
    let accepted = core.queue.pop_front().unwrap();
    assert!(matches!(
        accepted.operation,
        Operation::Write { value: false, .. }
    ));
    finish_write(&mut core, &accepted, Ok(()));
    let reread = core.queue.pop_front().unwrap();
    finish_read(&mut core, &reread, Ok(observed(Some(false))));
    assert!(!core.flags[&flag].value);
    let source_writer = core.queue.pop_front().unwrap();
    finish_write(&mut core, &source_writer, Ok(()));
    assert!(core.queue.is_empty());
}

#[test]
fn duplicate_notification_mounts_share_one_observer_and_remount_requires_a_new_read() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    mount_notifications(&mut core, "false", false);
    let generation = core.notification_watch.generation;
    core.observe_notifications();
    assert_eq!(core.notification_watch.generation, generation);
    assert!(core.queue.is_empty());
    core.cancel_observation(flag);
    core.observe_notifications();
    assert_ne!(core.notification_watch.generation, generation);
    assert_eq!(core.flags[&flag].load, LoadStatus::NotRequested);
    assert_eq!(core.flags[&flag].persisted_value, Some(false));
    let poll = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &poll, Ok(Some("true")));
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Ok(observed(Some(true))));
    assert!(core.flags[&flag].value);
}

#[test]
fn unmount_during_exit_does_not_resubscribe_after_canceling_exit() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    core.observe_notifications();
    let poll = core.queue.pop_front().unwrap();
    finish_poll(&mut core, &poll, Ok(Some("true")));
    let read = core.queue.pop_front().unwrap();
    core.begin_exit();
    core.cancel_observation(flag);
    core.cancel_exit();
    finish_read(&mut core, &read, Ok(observed(Some(true))));
    assert!(!core.notification_watch.active);
    assert_eq!(core.flags[&flag].load, LoadStatus::NotRequested);
    assert!(core.queue.is_empty());
}

#[test]
fn notification_mount_runs_source_writer_only_after_successful_observation() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    assert!(!core.flags[&flag].value);
    core.load(flag);
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Ok(observed(None)));
    assert!(core.flags[&flag].value);
    assert_eq!(core.flags[&flag].persisted_value, None);
    let write = core.queue.pop_front().unwrap();
    assert!(matches!(
        write.operation,
        Operation::Write { value: true, .. }
    ));
    finish_write(
        &mut core,
        &write,
        Err("app metadata publication failed".into()),
    );
    assert!(
        core.flags[&flag].value,
        "source optimistic editor is not rolled back"
    );
    assert_eq!(core.flags[&flag].persisted_value, None);
    assert!(core.save_error().is_some());
}

#[test]
fn failed_notification_read_has_no_default_write() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    core.load(flag);
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Err("non-boolean local value".into()));
    assert!(core.queue.is_empty());
    assert!(!core.flags[&flag].value);
    assert_eq!(core.flags[&flag].persisted_value, None);
}

#[test]
fn closed_notification_mount_cannot_start_an_initialization_write() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    core.load(flag);
    let read = core.queue.pop_front().unwrap();
    core.begin_exit();
    finish_read(&mut core, &read, Ok(observed(None)));
    assert!(core.queue.is_empty());
    assert_eq!(core.flags[&flag].load, LoadStatus::Cancelled);
}

#[test]
fn notification_unmount_rejects_late_reads_without_cancelling_accepted_writes_or_other_flags() {
    let mut core = Core::default();
    let flag = Flag::DisplayNotifications;
    core.load(flag);
    let old_read = core.queue.pop_front().unwrap();
    core.set(flag, false);
    core.load(Flag::HueTutorial);
    core.cancel_observation(flag);
    core.load(flag);
    finish_read(&mut core, &old_read, Ok(observed(Some(true))));
    assert!(!core.flags[&flag].value);
    assert_eq!(core.flags[&flag].load, LoadStatus::Loading);
    let write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &write, Ok(()));
    let other_read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &other_read, Ok(observed(Some(false))));
    assert!(!core.flags[&Flag::HueTutorial].value);
    let remount = core.queue.pop_front().unwrap();
    finish_read(&mut core, &remount, Ok(observed(Some(false))));
    assert_eq!(core.flags[&flag].persisted_value, Some(false));
    assert_eq!(
        core.queue.len(),
        1,
        "only the current mount may publish its writer"
    );
}

fn finish_read(core: &mut Core, work: &Work, result: Result<Observation, String>) {
    let Operation::Read {
        flag,
        revision,
        generation,
    } = work.operation
    else {
        panic!("expected queued read")
    };
    assert!(
        core.complete(Completed::Read {
            flag,
            revision,
            generation,
            result,
        })
        .is_none()
    );
}

#[gpui_kit::test]
fn initial_tab_work_is_serialized_and_remains_owned_through_exit(
    cx: &mut gpui_kit::TestAppContext,
) {
    cx.update(|cx| {
        let window_owner = cx.new(|_| ());
        let request = InitialTabRequest::new(window_owner.entity_id(), 1);
        let mut core = Core::default();
        core.set(Flag::HueTutorial, false);
        core.load(Flag::ArmoryIntroduction);
        assert!(core.record_initial_tab(request));
        core.begin_exit();
        assert!(!core.record_initial_tab(InitialTabRequest::new(window_owner.entity_id(), 2)));
        let first = core.queue.pop_front().unwrap();
        assert!(matches!(
            first.operation,
            Operation::Write {
                flag: Flag::HueTutorial,
                ..
            }
        ));
        finish_write(&mut core, &first, Ok(()));
        let tab = core.queue.pop_front().unwrap();
        assert!(
            matches!(tab.operation, Operation::InitialTab { request: found } if found == request)
        );
        assert_eq!(core.pending_writes, 1);
        core.complete(Completed::InitialTab(InitialTabFinished {
            request,
            result: Ok(InitialTabObservation::First {
                persistence: Err("disk full".into()),
            }),
        }));
        assert_eq!(core.pending_writes, 0);
        assert!(core.queue.is_empty());
        assert!(
            core.save_error().is_none(),
            "the guide's caught marker failure is not an unsaved settings draft"
        );
    });
}

fn finish_write(core: &mut Core, work: &Work, result: Result<(), String>) -> PersistenceFinished {
    let Operation::Write {
        flag,
        revision,
        value,
    } = work.operation
    else {
        panic!("expected queued write")
    };
    core.complete(Completed::Write {
        flag,
        revision,
        value,
        result,
    })
    .expect("a requested write produces its persistence receipt")
}

#[test]
fn a_late_load_observes_disk_without_overwriting_a_later_click() {
    let mut core = Core::default();
    core.load(Flag::ArmoryIntroduction);
    let read = core.queue.pop_front().unwrap();
    core.set(Flag::ArmoryIntroduction, false);
    finish_read(&mut core, &read, Ok(observed(Some(true))));
    let state = &core.flags[&Flag::ArmoryIntroduction];
    assert!(!state.value);
    assert_eq!(state.persisted_value, Some(true));
    assert_eq!(state.save, SaveStatus::Queued { revision: 1 });
    let write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &write, Ok(()));
    assert_eq!(
        core.flags[&Flag::ArmoryIntroduction].persisted_value,
        Some(false)
    );
}

#[test]
fn source_defaults_do_not_claim_a_successful_observation() {
    let mut core = Core::default();
    assert!(core.flags[&Flag::HueTutorial].value);
    assert!(!core.flags[&Flag::HueTutorial].ready());
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, None);
    core.load(Flag::HueTutorial);
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Err("malformed local file".into()));
    let state = &core.flags[&Flag::HueTutorial];
    assert!(state.value);
    assert!(state.ready());
    assert_eq!(
        state.load,
        LoadStatus::Failed("malformed local file".into())
    );
    assert_eq!(state.persisted_value, None);
}

#[test]
fn an_observed_missing_flag_is_distinct_from_a_failed_or_pending_load() {
    let mut core = Core::default();
    core.load(Flag::MagneticCalibrationSkipPrompt);
    core.load(Flag::MagneticCalibrationSkipPrompt);
    assert_eq!(core.queue.len(), 1);
    let read = core.queue.pop_front().unwrap();
    assert_eq!(
        core.flags[&Flag::MagneticCalibrationSkipPrompt].load,
        LoadStatus::Loading
    );
    finish_read(&mut core, &read, Ok(observed(None)));
    let state = &core.flags[&Flag::MagneticCalibrationSkipPrompt];
    assert!(state.ready());
    assert!(!state.value);
    assert_eq!(state.load, LoadStatus::Observed(observed(None)));
}

#[test]
fn writes_keep_acceptance_order_across_shared_pages_and_preferences() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    core.set(Flag::ArmoryIntroduction, false);
    core.set(Flag::HueTutorial, true);
    let first = core.queue.pop_front().unwrap();
    assert!(matches!(
        first.operation,
        Operation::Write {
            flag: Flag::HueTutorial,
            value: false,
            revision: 1
        }
    ));
    finish_write(&mut core, &first, Ok(()));
    assert!(core.flags[&Flag::HueTutorial].value);
    assert_eq!(
        core.flags[&Flag::HueTutorial].save,
        SaveStatus::Queued { revision: 2 }
    );
    let second = core.queue.pop_front().unwrap();
    assert!(matches!(
        second.operation,
        Operation::Write {
            flag: Flag::ArmoryIntroduction,
            value: false,
            revision: 1
        }
    ));
    finish_write(&mut core, &second, Ok(()));
    let third = core.queue.pop_front().unwrap();
    finish_write(&mut core, &third, Ok(()));
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, Some(true));
    assert_eq!(
        core.flags[&Flag::HueTutorial].save,
        SaveStatus::Saved { revision: 2 }
    );
}

#[test]
fn failed_writes_keep_the_optimistic_ui_and_previous_persisted_value() {
    let mut core = Core::default();
    core.load(Flag::KeyboardCalibrationIntro);
    let read = core.queue.pop_front().unwrap();
    finish_read(&mut core, &read, Ok(observed(Some(true))));
    core.set(Flag::KeyboardCalibrationIntro, false);
    let write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &write, Err("atomic publication failed".into()));
    let state = &core.flags[&Flag::KeyboardCalibrationIntro];
    assert!(!state.value);
    assert_eq!(state.persisted_value, Some(true));
    assert_eq!(
        state.save,
        SaveStatus::Failed {
            revision: 1,
            error: "atomic publication failed".into()
        }
    );
}

#[test]
fn earlier_write_failure_does_not_replace_a_newer_queued_revision() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    let first = core.queue.pop_front().unwrap();
    core.set(Flag::HueTutorial, true);
    finish_write(&mut core, &first, Err("first write failed".into()));
    let state = &core.flags[&Flag::HueTutorial];
    assert!(state.value);
    assert_eq!(state.save, SaveStatus::Queued { revision: 2 });
}

#[test]
fn exit_cancels_loads_rejects_new_work_and_drains_every_accepted_write() {
    let mut core = Core::default();
    core.load(Flag::HueTutorial);
    let in_flight = core.queue.pop_front().unwrap();
    core.load(Flag::ArmoryIntroduction);
    core.set(Flag::HueTutorial, false);
    core.set(Flag::ArmoryIntroduction, false);
    core.begin_exit();
    assert_eq!(core.queue.len(), 2);
    core.set(Flag::HueTutorial, true);
    core.load(Flag::MagneticCalibrationSkipPrompt);
    assert_eq!(core.queue.len(), 2);
    finish_read(&mut core, &in_flight, Ok(observed(Some(true))));
    assert_eq!(core.flags[&Flag::HueTutorial].load, LoadStatus::Cancelled);
    assert_eq!(
        core.flags[&Flag::ArmoryIntroduction].load,
        LoadStatus::Cancelled
    );
    while let Some(work) = core.queue.pop_front() {
        finish_write(&mut core, &work, Ok(()));
    }
    for flag in [Flag::HueTutorial, Flag::ArmoryIntroduction] {
        assert_eq!(core.flags[&flag].persisted_value, Some(false));
        assert_eq!(core.flags[&flag].save, SaveStatus::Saved { revision: 1 });
    }
}

#[test]
fn a_later_acquisition_retries_a_failed_read_and_retains_intervening_clicks() {
    let mut core = Core::default();
    core.load(Flag::ArmoryIntroduction);
    let first = core.queue.pop_front().unwrap();
    finish_read(&mut core, &first, Err("temporary read failure".into()));
    core.load(Flag::ArmoryIntroduction);
    let retry = core.queue.pop_front().unwrap();
    core.set(Flag::ArmoryIntroduction, false);
    finish_read(&mut core, &retry, Ok(observed(Some(true))));
    assert!(!core.flags[&Flag::ArmoryIntroduction].value);
    assert!(core.flags[&Flag::ArmoryIntroduction].ready());
}

#[test]
fn cancelling_exit_reopens_admission_and_rejects_the_cancelled_read_generation() {
    let mut core = Core::default();
    core.load(Flag::HueTutorial);
    let original = core.queue.pop_front().unwrap();
    core.set(Flag::HueTutorial, false);
    core.begin_exit();
    core.cancel_exit();
    assert!(!core.closing);
    assert_eq!(core.queue.len(), 2);
    finish_read(&mut core, &original, Ok(observed(Some(true))));
    assert_eq!(core.flags[&Flag::HueTutorial].load, LoadStatus::Loading);
    assert!(!core.flags[&Flag::HueTutorial].value);
    let accepted_write = core.queue.pop_front().unwrap();
    finish_write(&mut core, &accepted_write, Ok(()));
    let reread = core.queue.pop_front().unwrap();
    finish_read(&mut core, &reread, Ok(observed(Some(false))));
    assert_eq!(
        core.flags[&Flag::HueTutorial].load,
        LoadStatus::Observed(observed(Some(false)))
    );
    core.set(Flag::HueTutorial, true);
    assert_eq!(core.queue.len(), 1);
    assert_eq!(
        core.flags[&Flag::HueTutorial].save,
        SaveStatus::Queued { revision: 2 }
    );
}

#[test]
fn storage_panics_are_errors_and_a_subsequent_write_can_still_complete() {
    assert_eq!(
        guarded::<()>(|| panic!("simulated storage panic")),
        Err("UI preference storage panicked".into())
    );
    assert_eq!(guarded(|| Ok(42)), Ok(42));
}

#[test]
fn exit_write_failure_is_reported_until_an_explicit_retry_succeeds() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    let write = core.queue.pop_front().unwrap();
    core.begin_exit();
    let failed = finish_write(&mut core, &write, Err("disk is read-only".into()));
    assert_eq!(failed.flag(), Flag::HueTutorial);
    assert_eq!(failed.revision(), 1);
    assert!(!failed.is_success());
    assert_eq!(failed.error(), Some("disk is read-only"));
    assert_eq!(core.save_error(), Some("disk is read-only"));
    assert!(!core.flags[&Flag::HueTutorial].value);
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, None);
    assert!(core.queue.is_empty());
    assert_eq!(core.retry_failed(), 0);

    core.cancel_exit();
    assert_eq!(core.retry_failed(), 1);
    assert_eq!(core.save_error(), Some("disk is read-only"));
    assert_eq!(core.pending_writes, 1);
    assert_eq!(core.retry_failed(), 0);
    let retry = core.queue.pop_front().unwrap();
    let saved = finish_write(&mut core, &retry, Ok(()));
    assert!(saved.is_success());
    assert_eq!(saved.revision(), 2);
    assert_eq!(core.save_error(), None);
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, Some(false));
    assert_eq!(core.retry_failed(), 0);
}

#[test]
fn another_preference_success_cannot_confirm_a_pending_failed_write_retry() {
    let mut core = Core::default();
    core.set(Flag::HueTutorial, false);
    core.set(Flag::ArmoryIntroduction, false);
    let failed = core.queue.pop_front().unwrap();
    finish_write(&mut core, &failed, Err("Hue flag write failed".into()));
    assert_eq!(core.pending_writes, 1);
    assert_eq!(core.retry_failed(), 1);
    assert_eq!(core.pending_writes, 2);
    assert_eq!(core.save_error(), Some("Hue flag write failed"));

    let other = core.queue.pop_front().unwrap();
    let saved = finish_write(&mut core, &other, Ok(()));
    assert_eq!(saved.flag(), Flag::ArmoryIntroduction);
    assert!(saved.is_success());
    assert_eq!(core.pending_writes, 1);
    assert_eq!(core.save_error(), Some("Hue flag write failed"));
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, None);

    let retry = core.queue.pop_front().unwrap();
    let saved = finish_write(&mut core, &retry, Ok(()));
    assert_eq!(saved.flag(), Flag::HueTutorial);
    assert!(saved.is_success());
    assert_eq!(core.pending_writes, 0);
    assert_eq!(core.save_error(), None);
    assert_eq!(core.flags[&Flag::HueTutorial].persisted_value, Some(false));
}
